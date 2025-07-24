use std::{
    fmt::Display,
    sync::{Arc, LazyLock},
    time::Duration,
};

use dashmap::DashMap;
use jplearnbot::dictionary::{DictEntry, Kanji, NLevel, Pos, Reading, Sense};
use lazy_static::lazy_static;
use poise::{
    ChoiceParameter,
    serenity_prelude::{
        ComponentInteraction, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditMessage,
        UserId, http::Http,
    },
};
use rand::{
    rng,
    seq::{IndexedRandom, IteratorRandom, SliceRandom},
};
use regex::Regex;
use strum_macros::{EnumIter, EnumString};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::timeout,
};
use uuid::Uuid;

use crate::{
    config::Context,
    dictionary::Dictionary,
    emote::{self, Emote},
    image,
};

/// Game modes
#[derive(Debug, poise::ChoiceParameter, Clone, Copy)]
pub enum ModeChoice {
    #[name = "English ▶ ひらがな"]
    EngToHir,
    #[name = "ひらがな ▶ English"]
    HirToEng,
    #[name = "ひらがな ▶ 漢字"]
    HirToKan,
    #[name = "漢字 ▶ ひらがな"]
    KanToHir,
    #[name = "漢字 ▶ English"]
    KanToEng,
    #[name = "English ▶ 漢字"]
    EngToKan,
}

pub enum GameMessage {
    /// A component interaction.
    Interaction(ComponentInteraction),
    /// Indicates game should close.
    Close,
}

#[derive(Debug, Clone, Copy, EnumString, EnumIter, strum_macros::Display)]
pub enum PosFilter {
    #[strum(to_string = "Nouns 名詞")]
    Nouns,
    #[strum(to_string = "Verbs 動詞")]
    Verbs,
    #[strum(to_string = "Prenominals 敬称略")]
    Prenominals,
    #[strum(to_string = "Expression 表現")]
    Expressions,
    #[strum(to_string = "Conjunctions 接続詞")]
    Conjunctions,
    Other,
}

impl PosFilter {
    const fn as_pos(&self) -> &'static [Pos] {
        const NOUNS: [Pos; 7] = [
            Pos::N,
            Pos::NPr,
            Pos::NAdv,
            Pos::NPref,
            Pos::NSuf,
            Pos::NT,
            Pos::Pn,
        ];

        const VERBS: [Pos; 59] = [
            Pos::VUnspec,
            Pos::V1,
            Pos::V1S,
            Pos::V2aS,
            Pos::V2bK,
            Pos::V2bS,
            Pos::V2dK,
            Pos::V2dS,
            Pos::V2gk,
            Pos::V2gS,
            Pos::V2hK,
            Pos::V2hS,
            Pos::V2kK,
            Pos::V2kS,
            Pos::V2mK,
            Pos::V2mS,
            Pos::V2nS,
            Pos::V2rK,
            Pos::V2rS,
            Pos::V2sS,
            Pos::V2tK,
            Pos::V2tS,
            Pos::V2wS,
            Pos::V2yK,
            Pos::V2yS,
            Pos::V2zS,
            Pos::V4b,
            Pos::V4g,
            Pos::V4h,
            Pos::V4k,
            Pos::V4m,
            Pos::V4n,
            Pos::V4r,
            Pos::V4s,
            Pos::V4t,
            Pos::V5aru,
            Pos::V5b,
            Pos::V5g,
            Pos::V5k,
            Pos::V5kS,
            Pos::V5m,
            Pos::V5n,
            Pos::V5r,
            Pos::V5rI,
            Pos::V5s,
            Pos::V5t,
            Pos::V5u,
            Pos::V5uS,
            Pos::V5uru,
            Pos::Vi,
            Pos::Vk,
            Pos::Vn,
            Pos::Vr,
            Pos::Vs,
            Pos::VsC,
            Pos::VsI,
            Pos::VsS,
            Pos::Vt,
            Pos::Vz,
        ];

        const PRENOMINALS: [Pos; 3] = [Pos::AdjF, Pos::AdjPn, Pos::AdjNo];

        const EXPRESSIONS: [Pos; 2] = [Pos::Exp, Pos::Int];

        const CONJUNCTIONS: [Pos; 1] = [Pos::Conj];

        const OTHER: [Pos; 20] = [
            Pos::AdjI,
            Pos::AdjIx,
            Pos::AdjKari,
            Pos::AdjKu,
            Pos::AdjNa,
            Pos::AdjNari,
            Pos::AdjShiku,
            Pos::AdjT,
            Pos::Adv,
            Pos::AdvTo,
            Pos::Aux,
            Pos::AuxAdj,
            Pos::AuxV,
            Pos::Cop,
            Pos::Ctr,
            Pos::Num,
            Pos::Pref,
            Pos::Prt,
            Pos::Suf,
            Pos::Unc,
        ];

        match self {
            PosFilter::Nouns => &NOUNS,
            PosFilter::Verbs => &VERBS,
            PosFilter::Prenominals => &PRENOMINALS,
            PosFilter::Expressions => &EXPRESSIONS,
            PosFilter::Conjunctions => &CONJUNCTIONS,
            PosFilter::Other => &OTHER,
        }
    }
}

/// Manages all game sessions.
pub struct Manager {
    /// Handle to serenity client.
    http: Arc<Http>,
    /// Dictionary for getting randomized samples and entries.
    dictionary: Arc<Dictionary>,
    /// Stores transmitters to game sessions. A Server/DM may only have
    /// one active game session.
    sessions: Arc<DashMap<u64, Sender<GameMessage>>>,
}

impl Manager {
    pub fn new(http: Arc<Http>) -> Self {
        Manager {
            http,
            dictionary: Dictionary::new().into(),
            sessions: DashMap::new().into(),
        }
    }

    /// Starts a new game session with the selected `mode`, `levels`, and `pos`.
    /// A separate task is created for game interaction handling. A [`Sender`]
    /// to the session is stored in [`Self::sessions`] for the duration of the game.
    /// The sessions exists while there are words in the pool and user interaction
    /// doesn't timeout from inactivity. A session can be stopped prematurely by sending
    /// a [`GameMessage::Close`] through the sender.
    ///
    /// # Errors
    /// Fails if user already has an active game.
    pub fn start_game(
        &self,
        ctx: &Context<'_>,
        mode: ModeChoice,
        levels: Vec<NLevel>,
        filters: Vec<PosFilter>,
    ) -> Result<(), SessionAlreadyCreated> {
        let session_id = ctx
            .guild_id()
            .map(|g| g.get())
            .unwrap_or(ctx.author().id.get());

        if self.sessions.contains_key(&session_id) {
            return Err(SessionAlreadyCreated);
        }

        let channel_id = ctx.channel_id();

        let http = Arc::clone(&self.http);
        let sessions = Arc::clone(&self.sessions);
        let dictionary = Arc::clone(&self.dictionary);

        let mut pos = pos_filters_to_pos(filters);

        let (tx, mut rx) = mpsc::channel(10);
        self.sessions.insert(session_id, tx);

        tokio::spawn(async move {
            // Natural expected exit reason, reason may change from interactions or lack thereof.
            let mut exit_reason = InteractionExitReason::PoolExhausted;

            for (round, entry) in dictionary.sample(&levels, &pos).await.iter().enumerate() {
                pos.shuffle(&mut rng());
                let Some(question) = pos
                    .iter()
                    .find_map(|&p| Question::new(entry, mode, p, &dictionary))
                else {
                    continue;
                };

                let menu_id = format!("{session_id},{}", Uuid::new_v4());
                let mut menu = Menu::new(&http, menu_id, question, entry);

                if channel_id
                    .send_files(
                        &http,
                        menu.create_files(),
                        menu.create_message(round + 1, mode),
                    )
                    .await
                    .is_err()
                {
                    exit_reason = InteractionExitReason::NetworkError;
                    break;
                }

                if let Err(reason) = menu.handle_interactions(&mut rx).await {
                    exit_reason = reason;
                    break;
                }
            }

            let message = match exit_reason {
                InteractionExitReason::PoolExhausted => {
                    Some("There are no more words left in the pool")
                }
                InteractionExitReason::Timeout => Some("Stopping game due to inactivity..."),
                InteractionExitReason::NetworkError => {
                    Some("Stopping game due to network error...")
                }
                InteractionExitReason::CloseRequest => None,
            };

            if let Some(message) = message {
                channel_id
                    .send_message(&http, CreateMessage::new().content(message))
                    .await
                    .ok();
            }

            sessions.remove(&session_id);
        });

        Ok(())
    }

    /// Stops `session_id`'s game if it exists.
    ///
    /// Returns true if there was an active game stopped.
    ///
    /// Returns false if there was no game associated with the `session_id`.
    pub async fn stop(&self, session_id: u64) -> bool {
        if let Some(tx) = self.sessions.get(&session_id) {
            tx.send(GameMessage::Close).await.ok();
            return true;
        }

        false
    }

    /// Sends `interaction` to the game session compatible with the interaction's custom_id.
    /// Does nothing if no matching game sesssion.
    pub async fn send(&self, interaction: ComponentInteraction) {
        if let Some(tx) =
            parse_session_id(&interaction.data.custom_id).and_then(|id| self.sessions.get(&id))
        {
            tx.send(GameMessage::Interaction(interaction)).await.ok();
        }
    }
}

/// Converts [`PosFilter`]'s to [`Pos`] using [`PosFilter::as_pos`].
fn pos_filters_to_pos(filters: Vec<PosFilter>) -> Vec<Pos> {
    let mut res = Vec::new();

    for filter in filters {
        res.extend_from_slice(filter.as_pos());
    }

    res
}

/// Reasons listening for component interactions should stop.
enum InteractionExitReason {
    /// There are no more words left in the pool.
    PoolExhausted,
    /// Sender took too long to send a message.
    Timeout,
    /// Error sending data to Discord.
    NetworkError,
    /// Game should close.
    CloseRequest,
}

/// Extracts game session_id from interaction's custom_id.
fn parse_session_id(interaction_id: &str) -> Option<u64> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+").unwrap());

    let session_id = RE.find(interaction_id)?.as_str().parse().ok()?;

    Some(session_id)
}

#[derive(Debug)]
pub struct SessionAlreadyCreated;

impl Display for SessionAlreadyCreated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "User has already created a session")
    }
}

impl std::error::Error for SessionAlreadyCreated {}

/// Game question
struct Question {
    /// The word to translate.
    prompt: String,
    /// Possible translations of [`Self::prompt`].
    options: [String; 5],
    /// The index of the correct translation of [`Self::prompt`].
    answer: usize,
}

impl Question {
    fn new(entry: &DictEntry, mode: ModeChoice, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        match mode {
            ModeChoice::EngToHir => Self::new_eng_to_hir(entry, pos, dictionary),
            ModeChoice::HirToEng => Self::new_hir_to_eng(entry, pos, dictionary),
            ModeChoice::HirToKan => Self::new_hir_to_kan(entry, pos, dictionary),
            ModeChoice::KanToHir => Self::new_kan_to_hir(entry, pos, dictionary),
            ModeChoice::KanToEng => Self::new_kan_to_eng(entry, pos, dictionary),
            ModeChoice::EngToKan => Self::new_eng_to_kan(entry, pos, dictionary),
        }
    }

    fn new_eng_to_hir(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (reading, sense) = reading_sense_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = reading.text.clone();

        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }
                let (reading, _) = reading_sense_pair(e, pos)?;
                Some(reading.text.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options.iter().position(|o| reading.text == *o).unwrap();

        Some(Question {
            prompt: sense.gloss[0].content.clone(),
            options,
            answer,
        })
    }

    fn new_hir_to_eng(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (reading, sense) = reading_sense_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = sense.gloss[0].content.clone();

        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }
                let (_, sense) = reading_sense_pair(e, pos)?;
                Some(sense.gloss[0].content.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options
            .iter()
            .position(|o| sense.gloss[0].content == *o)
            .unwrap();

        Some(Question {
            prompt: reading.text.clone(),
            options,
            answer,
        })
    }

    fn new_hir_to_kan(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (kanji, reading) = kanji_reading_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = kanji.text.clone();
        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }

                let (kanji, _) = kanji_reading_pair(e, pos)?;
                Some(kanji.text.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options.iter().position(|o| kanji.text == *o).unwrap();

        Some(Question {
            prompt: reading.text.clone(),
            options,
            answer,
        })
    }

    fn new_kan_to_hir(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (kanji, reading) = kanji_reading_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = reading.text.clone();

        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }
                let (_, reading) = kanji_reading_pair(e, pos)?;
                Some(reading.text.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options.iter().position(|o| reading.text == *o).unwrap();

        Some(Question {
            prompt: kanji.text.clone(),
            options,
            answer,
        })
    }

    fn new_kan_to_eng(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (kanji, sense) = kanji_sense_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = sense.gloss[0].content.clone();

        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }
                let (_, sense) = kanji_sense_pair(e, pos)?;
                Some(sense.gloss[0].content.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options
            .iter()
            .position(|o| sense.gloss[0].content == *o)
            .unwrap();

        Some(Question {
            prompt: kanji.text.clone(),
            options,
            answer,
        })
    }

    fn new_eng_to_kan(entry: &DictEntry, pos: Pos, dictionary: &Dictionary) -> Option<Self> {
        let (kanji, sense) = kanji_sense_pair(entry, pos)?;

        let mut options = std::array::from_fn(|_| "".to_string());
        options[0] = kanji.text.clone();

        dictionary
            .entries
            .iter()
            .filter_map(|e| {
                if e.id == entry.id {
                    return None;
                }
                let (kanji, _) = kanji_sense_pair(e, pos)?;
                Some(kanji.text.clone())
            })
            .choose_multiple_fill(&mut rng(), &mut options[1..]);

        options.shuffle(&mut rng());

        let answer = options.iter().position(|o| kanji.text == *o).unwrap();

        Some(Question {
            prompt: sense.gloss[0].content.clone(),
            options,
            answer,
        })
    }
}

/// Conventiently extracts a [`Reading`] and correlated [`Sense`] from a [`DictEntry`] where
/// the sense has the `pos` tag and is guaranteed to have at least one gloss.
///
/// Returns [`None`] if no possible extraction.
fn reading_sense_pair(entry: &DictEntry, pos: Pos) -> Option<(&Reading, &Sense)> {
    let sense = entry
        .senses
        .iter()
        .find(|s| s.pos.contains(&pos) && !s.gloss.is_empty())?;

    let reading = entry
        .readings
        .iter()
        .find(|r| sense.relevant_reading.is_empty() || sense.relevant_reading.contains(&r.text))?;

    Some((reading, sense))
}

/// Conveniently extracts a [`Kanji`] and correlated [`Reading`] from a [`DictEntry`] where
/// the reading has the `pos` tag.
///
/// Returns [`None`] if no possible extraction.
fn kanji_reading_pair(entry: &DictEntry, pos: Pos) -> Option<(&Kanji, &Reading)> {
    let sense = entry.senses.iter().find(|s| s.pos.contains(&pos))?;

    let kanji = entry.kanjis.first()?;

    let reading = entry.readings.iter().find(|r| {
        (r.relevant_to.is_empty() || r.relevant_to.contains(&kanji.text))
            && (sense.relevant_reading.is_empty() || sense.relevant_reading.contains(&r.text))
    })?;

    Some((kanji, reading))
}

/// Conventiently extracts a [`Kanji`] and correlated [`Sense`] from a [`DictEntry`] where
/// the sense has the `pos` tag and is guaranteed to have at least one gloss.
///
/// Returns [`None`] if no possible extraction.
fn kanji_sense_pair(entry: &DictEntry, pos: Pos) -> Option<(&Kanji, &Sense)> {
    let sense = entry
        .senses
        .iter()
        .find(|s| s.pos.contains(&pos) && !s.gloss.is_empty())?;

    let kanji = entry.kanjis.first()?;

    Some((kanji, sense))
}

/// Manages the components of a game question.
struct Menu<'a> {
    id: String,
    prompt: String,
    questions: Vec<QuestionComponent>,
    answer: usize,
    entry: &'a DictEntry,
    http: &'a Http,
}

/// Contains data on a game button.
struct QuestionComponent {
    /// The component's unique identifier.
    id: String,
    /// Possible translation text.
    text: String,
    /// Whether this button should be disabled.
    disabled: bool,
}

impl<'a> Menu<'a> {
    fn new(http: &'a Http, id: String, question: Question, entry: &'a DictEntry) -> Self {
        let questions = question
            .options
            .into_iter()
            .enumerate()
            .map(|(i, text)| QuestionComponent {
                id: format!("{id},{i}"),
                text,
                disabled: false,
            })
            .collect();

        Menu {
            id,
            prompt: question.prompt,
            questions,
            answer: question.answer,
            entry,
            http,
        }
    }

    /// Collects all the levels of [`Self::entry`].
    fn levels(&self) -> Vec<NLevel> {
        let mut levels: Vec<_> = self
            .entry
            .readings
            .iter()
            .flat_map(|r| r.levels.clone())
            .collect();

        levels.sort_unstable();
        levels.dedup();

        levels
    }

    /// Equivalent to [`Self::questions`]\[\].[`id`]
    fn answer_id(&self) -> &str {
        &self.questions[self.answer].id
    }

    fn create_files(&self) -> Vec<CreateAttachment> {
        vec![CreateAttachment::bytes(
            image::text_to_image(&self.prompt),
            "prompt.png",
        )]
    }

    fn create_message(&self, round: usize, mode: ModeChoice) -> CreateMessage {
        CreateMessage::new()
            .embed(
                CreateEmbed::new()
                    .title(format!("Question {round}"))
                    .field(mode.name(), "", false)
                    .attachment("prompt.png"),
            )
            .components(self.create_components())
    }

    /// Create all of the components of this menu.
    fn create_components(&self) -> Vec<CreateActionRow> {
        let buttons = self
            .questions
            .iter()
            .map(|q| CreateButton::new(&q.id).label(&q.text).disabled(q.disabled))
            .collect();

        vec![CreateActionRow::Buttons(buttons)]
    }

    /// Listens for button interactions until the answer is chosen.
    async fn handle_interactions(
        &mut self,
        rx: &mut Receiver<GameMessage>,
    ) -> Result<(), InteractionExitReason> {
        loop {
            let mut ci = component_interaction(rx).await?;

            let Some((menu_id, choice)) = parse_custom_id(&ci.data.custom_id) else {
                continue;
            };
            // Skip if menu_id of previous round.
            if menu_id != self.id {
                continue;
            }

            let correct = self.questions[choice].id == self.answer_id();

            // If correct, disable all buttons since this round is finished.
            // Otherwise, just the wrongly selected button.
            if correct {
                self.questions.iter_mut().for_each(|q| q.disabled = true);
            } else {
                self.questions[choice].disabled = true;
            }

            ci.message
                .edit(
                    self.http,
                    EditMessage::new().components(self.create_components()),
                )
                .await
                .map_err(|_| InteractionExitReason::NetworkError)?;

            let message = if correct {
                const THUMBNAIL: &str = r"https://raw.githubusercontent.com/jasonly027/jplearnbot/dedaa826e9bbc942cf035ba8eeac15479e8d9416/assets/correct.png";
                let header = format!("{} {:?}", &self.questions[self.answer].text, self.levels());
                let body = format!(
                    "[**Definition ・ 意味**](https://jisho.org/search/{})\n{} {}",
                    urlencoding::encode(&self.questions[self.answer].text),
                    ci.user.name,
                    emote::emote.wow
                );

                CreateInteractionResponseMessage::new().embed(
                    CreateEmbed::new()
                        .title("Answer · 正解")
                        .thumbnail(THUMBNAIL)
                        .field(header, body, false),
                )
            } else {
                CreateInteractionResponseMessage::new()
                    .content(insult_message(ci.user.id, &self.questions[choice].text))
            };

            ci.create_response(self.http, CreateInteractionResponse::Message(message))
                .await
                .map_err(|_| InteractionExitReason::NetworkError)?;

            if correct {
                break;
            }
        }

        Ok(())
    }
}

/// Unwraps component interactions from `rx`.
///
/// Returns [`InteractionExitReason::Timeout`] if sender takes
/// too long.
///
/// Returns [`InteractionExitReason::CloseRequest`] if sender sends
/// [`GameMessage::Close`].
async fn component_interaction(
    rx: &mut Receiver<GameMessage>,
) -> Result<ComponentInteraction, InteractionExitReason> {
    let Ok(Some(msg)) = timeout(Duration::from_secs(120), rx.recv()).await else {
        return Err(InteractionExitReason::Timeout);
    };

    let GameMessage::Interaction(ci) = msg else {
        return Err(InteractionExitReason::CloseRequest);
    };

    Ok(ci)
}

/// Parses a component's custom_id for its menu_id and the user's button choice.
fn parse_custom_id(custom_id: &str) -> Option<(&str, usize)> {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*),([0-4])$").unwrap());

    RE.captures(custom_id)
        .and_then(|m| Some((m.get(1)?, m.get(2)?))) // Get capture groups
        .and_then(|(menu_id, choice)| {
            Some((menu_id.as_str(), choice.as_str().parse().ok()?)) // Convert format
        })
}

lazy_static! {
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
        } = *emote::emote;

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
fn insult_message(user_id: UserId, choice: &str) -> String {
    let insult = insults.choose(&mut rng()).unwrap();
    format!("{insult} <@{user_id}> ({choice})")
}
