use crate::regex::{custom_emoji_regex, url_regex};
use aho_corasick::{AhoCorasickBuilder, MatchKind};
use anyhow::Result;
use discord_md::generate::{ToMarkdownString, ToMarkdownStringOption};
use koe_db::{dict::GetAllOption, redis};
use serenity::{
    client::Context,
    model::{channel::Message, id::GuildId},
    utils::ContentSafeOptions,
};
use crate::app_state;

pub async fn build_read_text(
    ctx: &Context,
    conn: &mut redis::aio::Connection,
    guild_id: GuildId,
    msg: &Message,
    last_msg: &Option<Message>,
) -> Result<String> {
    let author_name = build_author_name(ctx, msg).await;
    let state = app_state::get(ctx).await?;

    let content = plain_content(ctx, msg);
    let content = replace_custom_emojis(&content);
    let content = discord_md::parse(&content).to_markdown_string(
        &ToMarkdownStringOption::new()
            .omit_format(true)
            .omit_spoiler(true),
    );
    let content = remove_url(&content);

    let text = if state.speak_user_name {
        if should_read_author_name(msg, last_msg) {
            format!("{}。{}", author_name, content)
        } else {
            content
        }
    } else {
        content
    };

    let text = replace_words_on_dict(conn, guild_id, &text).await?;

    // 文字数を制限
    if text.chars().count() > state.speak_length_limit {
        Ok(text.chars().take(state.speak_length_limit - 4).collect::<String>() + "、以下略")
    } else {
        Ok(text)
    }
}

fn should_read_author_name(msg: &Message, last_msg: &Option<Message>) -> bool {
    let last_msg = match last_msg {
        Some(msg) => msg,
        None => return true,
    };

    msg.author != last_msg.author
        || (msg.timestamp.unix_timestamp() - last_msg.timestamp.unix_timestamp()) > 10
}

async fn build_author_name(ctx: &Context, msg: &Message) -> String {
    msg.author_nick(&ctx.http)
        .await
        // FIXME: `User::name`はユーザーの表示名ではなく一意のユーザー名を返す。現在のSerenityの実装では、ユーザーの表示名を取得する方法がない。
        // cf. https://github.com/serenity-rs/serenity/discussions/2500
        .unwrap_or_else(|| msg.author.name.clone())
}

/// [Message]の内容を返す。ID表記されたメンションやチャンネル名は読める形に書き換える。
fn plain_content(ctx: &Context, msg: &Message) -> String {
    let mut options = ContentSafeOptions::new()
        .clean_channel(true)
        .clean_role(true)
        .clean_user(true)
        .show_discriminator(false)
        .clean_here(false)
        .clean_everyone(false);

    if let Some(guild_id) = msg.guild_id {
        options = options.display_as_member_from(guild_id);
    }

    serenity::utils::content_safe(&ctx.cache, &msg.content, &options, &msg.mentions)
}

/// カスタム絵文字を読める形に置き換える
fn replace_custom_emojis(text: &str) -> String {
    custom_emoji_regex().replace_all(text, "$1").into()
}

async fn replace_words_on_dict(
    conn: &mut redis::aio::Connection,
    guild_id: GuildId,
    text: &str,
) -> Result<String> {
    let dict = koe_db::dict::get_all(
        conn,
        GetAllOption {
            guild_id: guild_id.into(),
        },
    )
    .await?;

    let word_list = dict.iter().map(|(word, _)| word).collect::<Vec<_>>();
    let read_as_list = dict.iter().map(|(_, read_as)| read_as).collect::<Vec<_>>();

    let ac = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(word_list)?;

    Ok(ac.replace_all(text, &read_as_list))
}

/// メッセージのURLを除去
fn remove_url(text: &str) -> String {
    url_regex().replace_all(text, "、").into()
}
