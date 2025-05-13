use app::App;
use rusqlite::Connection;

mod app;
mod db;
mod view;

fn init_db() -> Option<Connection> {
	match db::init() {
		Ok(conn) => Some(conn),
		Err(err) => {
			println!("Sqlite Error: {err:?}");
			None
		}
	}
}

fn main() {
	let Some(conn) = init_db() else {
		return;
	};

	let terminal = ratatui::init();
	let result = App::new(conn).run(terminal);
	ratatui::restore();

	if let Err(err) = result {
		match err {
			app::AppError::Io(error) => println!("IO Error: {error}"),
			app::AppError::Sqlite(error) => println!("Sqlite Error: {error}"),
			app::AppError::SqliteMigration(error) => println!("Sqlite Error: {error}"),
			app::AppError::Encryption(str) => println!("Encryption error: {str}"),
		}
	}
}
