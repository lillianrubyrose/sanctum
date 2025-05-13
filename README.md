# Sanctum

An encrypted terminal-based journal and mood tracker application.

## Features

- Password-protected data leveraging XChaCha20Poly1305 for encryption
- Mood tracking with my [mood rating system](#mood-rating-system)
- SQLite database for data storage

## Usage

```
cargo run
```

When first launched, you'll be prompted to create a passphrase to encrypt your data.

## Navigation

Navigate the application using the keyboard:
- Arrow keys to move between fields
- Enter to confirm selections
- Escape to go back to previous screens

## Mood Rating System

The application uses a personal mood rating scale ranging from -10 to +10:
- 0 represents baseline "normal" or "okay"
- Positive values (up to +10) represent increasingly elevated moods, with +10 being manic
- Negative values (down to -10) represent increasingly depressed moods, with -10 being deep depression

The interpretation of this scale is subjective and can be adapted to your personal experience. This system was designed for my own personal use, and is what works for me.

There is no plan on supporting other mood rating systems at this time, as this application is first and foremost, for myself.

## Building

```
cargo build --release
```