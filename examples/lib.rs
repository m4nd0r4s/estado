//! Reference scenarios of spec section 11, one module per scenario.
//!
//! In milestone M2 they are written by hand against `estado-core`; in M3
//! they are rewritten with the macros and must pass the same tests.
pub mod a_http_parser;
pub mod a_http_parser_macro;
pub mod b_connection;
pub mod b_connection_macro;
pub mod d_game_character;
pub mod d_game_character_macro;
pub mod i_media_player;
pub mod i_media_player_macro;
