use serde::Deserialize;
use rusqlite::{Connection, Transaction};
use rusqlite::params;

#[derive(Deserialize, Debug)]
pub struct Record {
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub senses: Vec<Definition>,
    #[serde(default)]
    pub related: Vec<Word>,
    #[serde(default)]
    pub synonyms: Vec<Word>,
}

fn get_word_id(trans: &Transaction<'_>, word: impl AsRef<str>) -> rusqlite::Result<i64> {
    trans.query_row(r#"
            SELECT id FROM words WHERE word = ?;
        "#,
                    params![word.as_ref()],
                    |r| r.get(0),
    )
}

fn insert_word(trans: &Transaction<'_>, word: impl AsRef<str>) -> rusqlite::Result<()> {
    trans.execute(r#"INSERT OR IGNORE INTO words (word) VALUES (?)"#,
                  params![word.as_ref()],
    ).map(|_| ())
}

impl Record {
    pub fn write_to_db(&self, conn: &mut Connection) -> rusqlite::Result<()> {
        let trans = conn.transaction()?;

        insert_word(&trans, &self.word)?;
        let wid = get_word_id(&trans, &self.word)?;
        trace!("ID is {}", wid);

        self.senses.iter().try_for_each(
            |w| {
                w.glosses.iter().try_for_each(
                    |def| {
                        debug!("Adding definition: \"{}\"", def);
                        trans.execute(r#"
                    INSERT OR IGNORE INTO definitions VALUES (?, ?, ?);
                "#,
                                      params![&wid, &self.pos, def],
                        ).map(|_| ())
                    }
                )
            }
        )?;

        self.related.iter().try_for_each(
            |w| {
                insert_word(&trans, w)?;
                debug!("Adding related word: \"{}\"", w.as_ref());
                let oid = get_word_id(&trans, w)?;
                trans.execute("INSERT OR IGNORE INTO related_words VALUES (?, ?)",
                              params![&wid, &oid],
                ).map(|_| ())
            }
        )?;

        self.synonyms.iter().try_for_each(
            |w| {
                insert_word(&trans, w)?;
                debug!("Adding synonymous word: \"{}\"", w.as_ref());
                let oid = get_word_id(&trans, w)?;
                trans.execute("INSERT OR IGNORE INTO synonymous_words VALUES (?, ?)",
                              params![&wid, &oid],
                ).map(|_| ())
            }
        )?;

        trans.commit()
    }

    pub fn has_any_definitions(&self) -> bool {
        self.senses.iter().any(|d|d.len() > 0)
    }

    pub fn num_definitions(&self) -> usize {
        self.senses.iter().map(|s| s.len()).sum()
    }
}

#[derive(Deserialize, Debug)]
pub struct Definition {
    #[serde(default)]
    pub glosses: Vec<String>
}

impl Definition {
    pub fn len(&self) -> usize {
        self.glosses.len()
    }
}

#[derive(Deserialize, Debug)]
pub struct Word {
    pub word: String
}

impl AsRef<str> for Word {
    fn as_ref(&self) -> &str {
        &self.word
    }
}

impl From<Word> for String {
    fn from(w: Word) -> Self {
        w.word
    }
}