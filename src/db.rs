use chacha20poly1305::{
	AeadCore, KeyInit, XChaCha20Poly1305,
	aead::{Aead, OsRng, rand_core::RngCore as _},
};
use chrono::{DateTime, Duration, Local, TimeZone as _, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use rusqlite_migration::{M, Migrations};

use crate::app::{AppError, AppResult};

const MIGRATION_SLICE: &[M<'_>] = &[
	M::up(
		r"
CREATE TABLE IF NOT EXISTS config(
    enc_verification_hash BLOB,
    nonce BLOB,
    salt BLOB
);

CREATE TABLE IF NOT EXISTS mood_entries(
    id TEXT PRIMARY KEY,
    enc_mood_rating BLOB,
    enc_timestamp BLOB,
    enc_journal_entry BLOB
);
",
	),
	M::up(
		r"
CREATE TABLE IF NOT EXISTS journal_entries(
    id TEXT PRIMARY KEY,
    enc_timestamp BLOB,
    enc_journal_entry BLOB
);
",
	),
];
const MIGRATIONS: Migrations<'_> = Migrations::from_slice(MIGRATION_SLICE);

const VERIFICATION_HASH: &str =
	"FtyvzqIkm0^WjFgnGC2LOVE#3TG2yeKqrxsZYOU%s48SO!jtMUCH!B$y*JCELhKJES*5@RTITE&1%!y^62<3jQ94K";

#[derive(Clone)]
pub struct PartialMoodEntry {
	pub id: String,
	pub mood_rating: i8,
	pub formatted_date: String,
	pub timestamp: i64,
}

#[derive(Clone)]
pub struct PartialJournalEntry {
	pub id: String,
	pub formatted_date: String,
	pub timestamp: i64,
}

impl PartialMoodEntry {
	pub fn mood_rating_str(&self) -> String {
		self.mood_rating.to_string()
	}

	pub fn formatted_date(&self) -> &str {
		&self.formatted_date
	}
}

impl PartialJournalEntry {
	pub fn formatted_date(&self) -> &str {
		&self.formatted_date
	}
}

fn generate_random_id() -> String {
	const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

	let mut id = String::with_capacity(64);

	for _ in 0..64 {
		let alphabet_len = u32::try_from(ALPHABET.len()).expect("alphabet length will fit in u32");
		let idx = (OsRng.next_u32() % alphabet_len) as usize;
		id.push(ALPHABET[idx] as char);
	}

	id
}

pub fn init() -> AppResult<Connection> {
	let mut conn = Connection::open("./sanctum.db").map_err(AppError::Sqlite)?;
	MIGRATIONS.to_latest(&mut conn).map_err(AppError::SqliteMigration)?;
	Ok(conn)
}

pub fn needs_first_setup(conn: &mut Connection) -> AppResult<bool> {
	let encrypted_hash: Option<Vec<u8>> = conn
		.query_row("SELECT enc_verification_hash FROM config LIMIT 1", (), |row| row.get(0))
		.optional()
		.map_err(AppError::Sqlite)?;

	Ok(encrypted_hash.is_none())
}

pub fn get_encryption_cipher(conn: &mut Connection, passphrase: &str) -> AppResult<Option<XChaCha20Poly1305>> {
	let (encrypted_hash, nonce, salt): (Vec<u8>, Vec<u8>, Vec<u8>) = conn
		.query_row(
			"SELECT enc_verification_hash, nonce, salt FROM config LIMIT 1",
			(),
			|row| {
				let enc_hash: Vec<u8> = row.get(0)?;
				let nonce: Vec<u8> = row.get(1)?;
				let salt: Vec<u8> = row.get(2)?;
				Ok((enc_hash, nonce, salt))
			},
		)
		.map_err(AppError::Sqlite)?;

	let mut key = [0u8; 32];
	let argon2 = argon2::Argon2::default();
	argon2
		.hash_password_into(passphrase.as_bytes(), &salt, &mut key)
		.expect("argon2 to not fail");

	let cipher = XChaCha20Poly1305::new(&key.into());

	let Ok(decrypted_hash) = cipher.decrypt(nonce.as_slice().into(), encrypted_hash.as_ref()) else {
		return Ok(None);
	};

	if decrypted_hash == VERIFICATION_HASH.as_bytes() {
		Ok(Some(cipher))
	} else {
		Ok(None)
	}
}

pub fn set_passphrase_initial(conn: &mut Connection, passphrase: &str) -> AppResult<XChaCha20Poly1305> {
	let mut salt = [0u8; 16];
	OsRng.fill_bytes(&mut salt);

	let mut key = [0u8; 32];
	let argon2 = argon2::Argon2::default();
	argon2
		.hash_password_into(passphrase.as_bytes(), &salt, &mut key)
		.expect("argon2 to not fail");

	let cipher = XChaCha20Poly1305::new(&key.into());
	let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

	let encrypted_hash = cipher.encrypt(&nonce, VERIFICATION_HASH.as_bytes()).unwrap();

	conn.execute(
		"INSERT INTO config (enc_verification_hash, nonce, salt) VALUES (?, ?, ?)",
		params![encrypted_hash, nonce.as_slice(), salt],
	)
	.map_err(AppError::Sqlite)?;

	Ok(cipher)
}

pub fn save_mood_entry(
	conn: &mut Connection,
	cipher: &XChaCha20Poly1305,
	mood_rating: i8,
	journal_entry: &str,
	time_offset_hours: i8,
) -> AppResult<PartialMoodEntry> {
	let timestamp = Utc::now() - Duration::hours(i64::from(time_offset_hours).abs());
	let timestamp_local = DateTime::<Local>::from(timestamp);
	let timestamp: i64 = timestamp.timestamp();

	let mood_nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
	let timestamp_nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
	let journal_nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

	let mut enc_mood_rating = Vec::with_capacity(24 + 1 + 16); // nonce + i8 + tag
	enc_mood_rating.extend_from_slice(mood_nonce.as_slice());
	enc_mood_rating.extend_from_slice(
		&cipher
			.encrypt(&mood_nonce, mood_rating.to_le_bytes().as_slice())
			.map_err(|err| AppError::Encryption(format!("Failed to decrypt mood rating: {err}")))?,
	);

	let mut enc_timestamp = Vec::with_capacity(24 + 8 + 16); // nonce + i64 + tag
	enc_timestamp.extend_from_slice(timestamp_nonce.as_slice());
	enc_timestamp.extend_from_slice(
		&cipher
			.encrypt(&timestamp_nonce, timestamp.to_le_bytes().as_slice())
			.map_err(|err| AppError::Encryption(format!("Failed to encrypt timestamp: {err}")))?,
	);

	let journal_bytes = journal_entry.as_bytes();
	let mut enc_journal_entry = Vec::with_capacity(24 + journal_bytes.len() + 16); // nonce + journal + tag
	enc_journal_entry.extend_from_slice(journal_nonce.as_slice());
	enc_journal_entry.extend_from_slice(
		&cipher
			.encrypt(&journal_nonce, journal_bytes)
			.map_err(|err| AppError::Encryption(format!("Failed to encrypt journal: {err}")))?,
	);

	let id = generate_random_id();

	conn.execute(
		"INSERT INTO mood_entries (id, enc_mood_rating, enc_timestamp, enc_journal_entry) VALUES (?, ?, ?, ?)",
		params![id, enc_mood_rating, enc_timestamp, enc_journal_entry],
	)
	.map_err(AppError::Sqlite)?;

	let formatted_date = timestamp_local.format("%m/%d/%Y %H:%M").to_string();

	Ok(PartialMoodEntry {
		id,
		mood_rating,
		formatted_date,
		timestamp,
	})
}

pub fn get_all_mood_entries(conn: &mut Connection, cipher: &XChaCha20Poly1305) -> AppResult<Vec<PartialMoodEntry>> {
	let mut stmt = conn
		.prepare("SELECT id, enc_mood_rating, enc_timestamp FROM mood_entries")
		.map_err(AppError::Sqlite)?;

	let rows = stmt
		.query_map([], |row| {
			let id: String = row.get(0)?;
			let enc_mood_rating: Vec<u8> = row.get(1)?;
			let enc_timestamp: Vec<u8> = row.get(2)?;

			Ok((id, enc_mood_rating, enc_timestamp))
		})
		.map_err(AppError::Sqlite)?;

	let approx_count = conn
		.query_row("SELECT COUNT(*) FROM mood_entries", [], |row| {
			let count: i32 = row.get(0)?;
			Ok(usize::try_from(count).expect("count will fit in usize"))
		})
		.unwrap_or(10);

	let mut entries = Vec::with_capacity(approx_count);

	for row_result in rows {
		let (id, enc_mood_rating, enc_timestamp) = row_result.map_err(AppError::Sqlite)?;

		let mood_nonce = enc_mood_rating
			.get(..24)
			.ok_or_else(|| AppError::Encryption("Failed to get mood rating nonce".to_string()))?;
		let timestamp_nonce = enc_timestamp
			.get(..24)
			.ok_or_else(|| AppError::Encryption("Failed to get timestamp nonce".to_string()))?;

		let mood_ciphertext = enc_mood_rating
			.get(24..)
			.ok_or_else(|| AppError::Encryption("Failed to get mood rating ciphertext".to_string()))?;
		let timestamp_ciphertext = enc_timestamp
			.get(24..)
			.ok_or_else(|| AppError::Encryption("Failed to get timestamp ciphertext".to_string()))?;

		let mood_rating_bytes = cipher
			.decrypt(mood_nonce.into(), mood_ciphertext)
			.map_err(|err| AppError::Encryption(format!("Failed to decrypt mood rating: {err}")))?;
		let timestamp_bytes = cipher
			.decrypt(timestamp_nonce.into(), timestamp_ciphertext)
			.map_err(|err| AppError::Encryption(format!("Failed to decrypt timestamp: {err}")))?;

		let mood_rating = i8::from_le_bytes(
			mood_rating_bytes
				.as_slice()
				.try_into()
				.expect("stored mood rating to be a single signed byte"),
		);
		let timestamp = i64::from_le_bytes(
			timestamp_bytes
				.as_slice()
				.try_into()
				.expect("stored timestamp to be an LE i64"),
		);

		let formatted_date = Utc.timestamp_opt(timestamp, 0).single().expect("valid timestamp");
		let formatted_date: DateTime<Local> = DateTime::<Local>::from(formatted_date);
		let formatted_date = formatted_date.format("%m/%d/%Y %H:%M").to_string();

		entries.push(PartialMoodEntry {
			id,
			mood_rating,
			formatted_date,
			timestamp,
		});
	}

	entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

	Ok(entries)
}

pub fn save_journal_entry(
	conn: &mut Connection,
	cipher: &XChaCha20Poly1305,
	journal_entry: &str,
	time_offset_hours: i8,
) -> AppResult<PartialJournalEntry> {
	let timestamp = Utc::now() - Duration::hours(i64::from(time_offset_hours).abs());
	let timestamp_local = DateTime::<Local>::from(timestamp);
	let timestamp: i64 = timestamp.timestamp();

	let timestamp_nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
	let journal_nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

	let mut enc_timestamp = Vec::with_capacity(24 + 8 + 16); // nonce + i64 + tag
	enc_timestamp.extend_from_slice(timestamp_nonce.as_slice());
	enc_timestamp.extend_from_slice(
		&cipher
			.encrypt(&timestamp_nonce, timestamp.to_le_bytes().as_slice())
			.map_err(|err| AppError::Encryption(format!("Failed to encrypt timestamp: {err}")))?,
	);

	let journal_bytes = journal_entry.as_bytes();
	let mut enc_journal_entry = Vec::with_capacity(24 + journal_bytes.len() + 16); // nonce + journal + tag
	enc_journal_entry.extend_from_slice(journal_nonce.as_slice());
	enc_journal_entry.extend_from_slice(
		&cipher
			.encrypt(&journal_nonce, journal_bytes)
			.map_err(|err| AppError::Encryption(format!("Failed to encrypt journal: {err}")))?,
	);

	let id = generate_random_id();

	conn.execute(
		"INSERT INTO journal_entries (id, enc_timestamp, enc_journal_entry) VALUES (?, ?, ?)",
		params![id, enc_timestamp, enc_journal_entry],
	)
	.map_err(AppError::Sqlite)?;

	let formatted_date = timestamp_local.format("%m/%d/%Y %H:%M").to_string();

	Ok(PartialJournalEntry {
		id,
		formatted_date,
		timestamp,
	})
}

pub fn get_all_journal_entries(
	conn: &mut Connection,
	cipher: &XChaCha20Poly1305,
) -> AppResult<Vec<PartialJournalEntry>> {
	let mut stmt = conn
		.prepare("SELECT id, enc_timestamp FROM journal_entries")
		.map_err(AppError::Sqlite)?;

	let rows = stmt
		.query_map([], |row| {
			let id: String = row.get(0)?;
			let enc_timestamp: Vec<u8> = row.get(1)?;

			Ok((id, enc_timestamp))
		})
		.map_err(AppError::Sqlite)?;

	let approx_count = conn
		.query_row("SELECT COUNT(*) FROM journal_entries", [], |row| {
			let count: i32 = row.get(0)?;
			Ok(usize::try_from(count).expect("count will fit in usize"))
		})
		.unwrap_or(10);

	let mut entries = Vec::with_capacity(approx_count);

	for row_result in rows {
		let (id, enc_timestamp) = row_result.map_err(AppError::Sqlite)?;

		let timestamp_nonce = enc_timestamp
			.get(..24)
			.ok_or_else(|| AppError::Encryption("Failed to get timestamp nonce".to_string()))?;

		let timestamp_ciphertext = enc_timestamp
			.get(24..)
			.ok_or_else(|| AppError::Encryption("Failed to get timestamp ciphertext".to_string()))?;

		let timestamp_bytes = cipher
			.decrypt(timestamp_nonce.into(), timestamp_ciphertext)
			.map_err(|err| AppError::Encryption(format!("Failed to decrypt timestamp: {err}")))?;

		let timestamp = i64::from_le_bytes(
			timestamp_bytes
				.as_slice()
				.try_into()
				.expect("stored timestamp to be an LE i64"),
		);

		let formatted_date = Utc.timestamp_opt(timestamp, 0).single().expect("valid timestamp");
		let formatted_date: DateTime<Local> = DateTime::<Local>::from(formatted_date);
		let formatted_date = formatted_date.format("%m/%d/%Y %H:%M").to_string();

		entries.push(PartialJournalEntry {
			id,
			formatted_date,
			timestamp,
		});
	}

	entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

	Ok(entries)
}

pub fn get_journal_text(
	conn: &mut Connection,
	cipher: &XChaCha20Poly1305,
	entry_id: &str,
	is_mood_entry: bool,
) -> AppResult<String> {
	let table_name = if is_mood_entry {
		"mood_entries"
	} else {
		"journal_entries"
	};

	let query = format!("SELECT enc_journal_entry FROM {table_name} WHERE id = ?");

	let enc_journal_entry: Vec<u8> = conn
		.query_row(&query, params![entry_id], |row| row.get(0))
		.map_err(AppError::Sqlite)?;

	let journal_nonce = enc_journal_entry
		.get(..24)
		.ok_or_else(|| AppError::Encryption("Failed to get journal nonce".to_string()))?;

	let journal_ciphertext = enc_journal_entry
		.get(24..)
		.ok_or_else(|| AppError::Encryption("Failed to get journal ciphertext".to_string()))?;

	let journal_bytes = cipher
		.decrypt(journal_nonce.into(), journal_ciphertext)
		.map_err(|err| AppError::Encryption(format!("Failed to decrypt journal: {err}")))?;

	String::from_utf8(journal_bytes)
		.map_err(|err| AppError::Encryption(format!("Failed to decode journal from UTF-8: {err}")))
}
