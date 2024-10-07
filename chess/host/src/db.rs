use rusqlite::{params, types::ToSql, Connection, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json;
use shakmaty::{fen::Fen, CastlingMode, Chess};
use uuid::Uuid;

pub struct DbManager {
    connection: Connection,
}

impl DbManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let connection = Connection::open(db_path)?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS games (
                game_id TEXT PRIMARY KEY,
                game_data TEXT NOT NULL,
                last_update INTEGER NOT NULL
            )",
            [],
        )?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS player_matchups (
                player1 TEXT NOT NULL,
                player2 TEXT NOT NULL,
                game_id TEXT NOT NULL,
                PRIMARY KEY (player1, player2),
                FOREIGN KEY (game_id) REFERENCES games(game_id)
            )",
            [],
        )?;

        connection.execute(
            "CREATE TABLE IF NOT EXISTS emails (
                game_id TEXT PRIMARY KEY,
                raw_email BLOB NOT NULL,
                FOREIGN KEY (game_id) REFERENCES games(game_id)
            )",
            [],
        )?;
        Ok(Self { connection })
    }

    pub fn store_email(&self, game_id: &str, raw_email: &[u8]) -> Result<()> {
        self.connection.execute(
            "INSERT OR REPLACE INTO emails (game_id, raw_email) VALUES (?1, ?2)",
            params![game_id, raw_email],
        )?;
        Ok(())
    }

    pub fn get_email(&self, game_id: &str) -> Result<Option<Vec<u8>>> {
        let mut stmt = self
            .connection
            .prepare("SELECT raw_email FROM emails WHERE game_id = ?")?;
        let mut rows = stmt.query([game_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn store_game(&self, game_id: &str, game: &Game) -> Result<()> {
        let serialized_game = serde_json::to_string(game)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let timestamp = chrono::Utc::now().timestamp();
        self.connection.execute(
            "INSERT OR REPLACE INTO games (game_id, game_data, last_update)
             VALUES (?1, ?2, ?3)",
            params![game_id, serialized_game, timestamp],
        )?;
        println!("stored game: {}", game_id);
        Ok(())
    }

    pub fn get_game(&self, game_id: &str) -> Result<Option<Game>> {
        let mut stmt = self
            .connection
            .prepare("SELECT game_data FROM games WHERE game_id = ?")?;
        let mut rows = stmt.query([game_id])?;
        if let Some(row) = rows.next()? {
            let game_json: String = row.get(0)?;
            let game: Game = serde_json::from_str(&game_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(Some(game))
        } else {
            Ok(None)
        }
    }

    pub fn delete_game(&self, game_id: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM games WHERE game_id = ?", [game_id])?;
        self.connection
            .execute("DELETE FROM player_matchups WHERE game_id = ?", [game_id])?;
        Ok(())
    }

    pub fn get_game_id(&self, player1: &str, player2: &str) -> Result<String> {
        let mut stmt = self.connection.prepare(
            "SELECT game_id FROM player_matchups 
             WHERE (player1 = ?1 AND player2 = ?2) OR (player1 = ?2 AND player2 = ?1)",
        )?;
        let mut rows = stmt.query(params![player1, player2])?;

        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            let new_game_id = Uuid::new_v4().to_string();
            self.connection.execute(
                "INSERT INTO player_matchups (player1, player2, game_id) VALUES (?1, ?2, ?3)",
                params![player1, player2, new_game_id],
            )?;
            Ok(new_game_id)
        }
    }
}

enum Color {
    Black,
    White,
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Color::White => serializer.serialize_str("White"),
            Color::Black => serializer.serialize_str("Black"),
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "White" => Ok(Color::White),
            "Black" => Ok(Color::Black),
            _ => Err(serde::de::Error::custom("Invalid color")),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Player {
    email: String,
    color: Color,
    is_next: bool,
}

impl Player {
    pub fn new_turn(&mut self) {
        self.is_next = !self.is_next;
    }
}

enum GameState {
    InProgress,
    Checkmate,
    Stalemate,
    Draw,
}

impl Serialize for GameState {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            GameState::InProgress => serializer.serialize_str("InProgress"),
            GameState::Checkmate => serializer.serialize_str("Checkmate"),
            GameState::Stalemate => serializer.serialize_str("Stalemate"),
            GameState::Draw => serializer.serialize_str("Draw"),
        }
    }
}

impl<'de> Deserialize<'de> for GameState {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "InProgress" => Ok(GameState::InProgress),
            "Checkmate" => Ok(GameState::Checkmate),
            "Stalemate" => Ok(GameState::Stalemate),
            "Draw" => Ok(GameState::Draw),
            _ => Err(serde::de::Error::custom("Invalid game state")),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Game {
    pub position: SerializedChess,
    players: [Player; 2],
    state: GameState,
}

impl Game {
    pub fn new(player_1: &str, player_2: &str) -> Self {
        let position = SerializedChess {
            chess: Chess::default(),
        };
        Self {
            position,
            players: [
                Player {
                    email: player_1.to_string(),
                    color: Color::White,
                    is_next: false,
                },
                Player {
                    email: player_2.to_string(),
                    color: Color::White,
                    is_next: false,
                },
            ],
            state: GameState::InProgress,
        }
    }

    pub fn update(&mut self, new_position: &Chess) {
        self.players[0].new_turn();
        self.players[1].new_turn();

        self.position = SerializedChess {
            chess: new_position.clone(),
        };
    }
}

pub struct SerializedChess {
    pub chess: Chess,
}

impl Serialize for SerializedChess {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let fen =
            Fen::from_position(self.chess.clone(), shakmaty::EnPassantMode::Legal).to_string();
        serializer.serialize_str(&fen)
    }
}

impl<'de> Deserialize<'de> for SerializedChess {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fen = String::deserialize(deserializer)?;
        let chess = Fen::from_ascii(fen.as_bytes())
            .map_err(serde::de::Error::custom)?
            .into_position(CastlingMode::Standard)
            .map_err(serde::de::Error::custom)?;
        Ok(SerializedChess { chess })
    }
}
