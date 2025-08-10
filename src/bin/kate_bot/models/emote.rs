use lazy_static::lazy_static;
use poise::serenity_prelude::UserId;
use rand::{rng, seq::IndexedRandom};

use crate::config::{Env, environment};

pub struct Emote {
    pub wow: &'static str,
    pub fubu_laugh: &'static str,
    pub scrajj: &'static str,
    pub anw: &'static str,
    pub wat: &'static str,
    pub maji: &'static str,
    pub ee: &'static str,
    pub baaka: &'static str,
    pub manuke: &'static str,
    pub wawawa: &'static str,
    pub hehe: &'static str,
    pub hayaku: &'static str,
    pub goofyahh: &'static str,
}

lazy_static! {
    pub static ref emote: Emote = match environment() {
        Env::Dev => {
            Emote {
                wow: "<:wow:1376760017486741544>",
                fubu_laugh: "<:fubu_laugh:1375302817778106490>",
                scrajj: "<a:scrajj:1375305497267146874>",
                anw: "<a:aintnoway:1375305628444004473>",
                wat: "<:wat:1373080615313739858>",
                maji: "<:maji:1398028508160065717>",
                ee: "<:ee:1398028499058429952>",
                baaka: "<:baaka:1398028489654800424>",
                manuke: "<:manuke:1398028480620396655>",
                wawawa: "<:wawawa:1398028471095001352>",
                hehe: "<:hehe:1398028460290736159>",
                hayaku: "<:hayaku:1398028448022401025>",
                goofyahh: "<a:goofyahh:1398031476917796894>",
            }
        }
        Env::Prod => {
            Emote {
                wow: "<:wow:1377207933930049608>",
                fubu_laugh: "<:fubu_laugh:1377208303460941864>",
                scrajj: "<a:scrajj:1377208217783894118>",
                anw: "<a:aintnoway:1377208154265485385>",
                wat: "<:wat:1377208406460469338>",
                maji: "<:maji:1398029826618691675>",
                ee: "<:ee:1398029819501084672>",
                baaka: "<:baaka:1398029811276058786>",
                manuke: "<:manuke:1398029802631467048>",
                wawawa: "<:wawawa:1398029792841830400>",
                hehe: "<:hehe:1398029785187221636>",
                hayaku: "<:hayaku:1398029776236581119>",
                goofyahh: "<a:goofyahh:1398031608661016780>>",
            }
        }
    };

    static ref insults: [String; 28] = {
        let Emote {
            wow: _wow,
            fubu_laugh,
            scrajj,
            anw,
            wat,
            maji,
            ee,
            baaka,
            manuke,
            wawawa,
            hehe,
            hayaku,
            goofyahh,
        } = *emote;

        [
            format!("{wat} noob"),
            format!("{wat} nuh-uh"),
            format!("{wat} what is he cooking"),
            format!("{wat} refund nitro"),
            format!("{wat} trolling are we?"),
            format!("{wat} nt bro"),
            format!("{wat} smooth brain"),
            format!("{wat} stop"),
            format!("{wat} ?"),
            format!("{wat} so bad"),
            format!("{wat} meow"),
            format!("{wat} imagine"),
            format!("{wat} no"),
            format!("{wat} wrong"),
            format!("{wat} ぴえん"),
            format!("{wat} あほ"),
            wat.to_string(),
            fubu_laugh.to_string(),
            scrajj.to_string(),
            anw.to_string(),
            format!("{maji} マ？"),
            format!("{ee} え？"),
            format!("{baaka} ばーか！"),
            format!("{manuke} アンタがまぬけ？"),
            format!("{wawawa} How?"),
            format!("{hehe} あほ！"),
            format!("{hayaku} 早く！"),
            format!("{}{}{}", goofyahh, goofyahh, goofyahh),
        ]
    };
}

/// Creates a randomized insult message that mentions `user_id`.
pub fn insult_message(user_id: UserId, choice: &str) -> String {
    let insult = insults.choose(&mut rng()).unwrap();
    format!("{insult} <@{user_id}> ({choice})")
}
