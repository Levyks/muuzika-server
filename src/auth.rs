use std::collections::HashSet;

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::MuuzikaResult;
use crate::rooms::{RoomCode, Username};

#[derive(Serialize, Deserialize, Debug)]
pub struct JwtClaims {
    pub iat: u64,
    pub room_code: RoomCode,
    pub username: Username,
}

pub fn encode_token(
    secret: &String,
    iat: u64,
    room_code: &RoomCode,
    username: &Username,
) -> MuuzikaResult<String> {
    let claims = JwtClaims {
        iat,
        room_code: room_code.clone(),
        username: username.clone(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

pub fn decode_token(secret: &String, token: &String) -> MuuzikaResult<JwtClaims> {
    let mut validation = Validation::default();
    validation.validate_exp = false;
    validation.required_spec_claims = HashSet::with_capacity(0);

    let claims = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;

    Ok(claims.claims)
}
