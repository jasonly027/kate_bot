use poise::{CreateReply, serenity_prelude::CreateEmbed};

use crate::{config::Context, Error};

/// Information about the bot.
#[poise::command(
    slash_command,
    user_cooldown = 3,
    name_localized("ja", "情報"),
    description_localized("ja", "ボットの情報")
)]
pub async fn info(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(
        CreateReply::default()
        .content("Art - <https://x.com/matcha__ore_p/>\nQuestions/Feedback - (discord) sweetenedlegs")
        .embed(CreateEmbed::new()
                .field( "Game Modes - (displayed text) ▶ (answer type)",
                    "· Displayed text will be presented as an image as the question.\n· Answer type is the displayed available answer buttons.",
                    false
                )
                .field("JLPT Levels - (N4 - N1)",
                    "· N-Level measures the level of understanding of basic Japanese. Unfortunately the source material did not include N5.",
                    false
                )
                .field("Part-of-speech Categories",
                    "· The `Other` category contains the remaining part-of-speech words that do not fit the other classifications.",
                    false
                )
        )
        .embed(CreateEmbed::new()
                .field("ゲームモード - (表示テキスト) ▷ (回答タイプ)",
                    "· 表示テキストは、問題として画像で提示されます。\n· 回答タイプは、選択可能な回答ボタンとして表示されます。",
                    false
                )
                .field("JLPTレベル - (N4 - N1)",
                    "· Nレベルは、基本的な日本語の理解度を測るものです。残念ながら、N5の教材は含まれていません。",
                    false
                )
                .field("品詞カテゴリ",
                    "· 「その他」カテゴリには、他の分類に当てはまらない残りの品詞の単語が含まれます。",
                    false
                )
            )
    )
    .await?;

    Ok(())
}
