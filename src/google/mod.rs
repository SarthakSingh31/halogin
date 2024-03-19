use std::{sync::LazyLock, time::Instant};

use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
    revocation::StandardRevocableToken,
    AccessToken, AuthUrl, Client, ClientId, ClientSecret, ExtraTokenFields, RedirectUrl,
    RefreshToken, StandardTokenResponse, TokenResponse, TokenUrl,
};

static AUTH_URL: LazyLock<AuthUrl> = LazyLock::new(|| {
    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".into())
        .expect("Failed to parse account url")
});
static TOKEN_URL: LazyLock<TokenUrl> = LazyLock::new(|| {
    TokenUrl::new("https://oauth2.googleapis.com/token".into()).expect("Failed to parse token url")
});

type GoogleClient = Client<
    BasicErrorResponse,
    StandardTokenResponse<IdToken, BasicTokenType>,
    BasicTokenType,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
>;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdToken {
    id_token: String,
}

impl ExtraTokenFields for IdToken {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IdTokenDecoded {
    sub: String,
}

#[derive(Debug, Clone)]
pub struct GoogleSession {
    bearer_access_token: AccessToken,
    expires_at_and_refresh_token: Option<(Instant, RefreshToken)>,
    sub: String,
}

impl GoogleSession {
    const CLIENT_ID: &'static str =
        "751704262503-61e56pavvl5d8l5fg6s62iejm8ft16ac.apps.googleusercontent.com";
    const CLIENT_SECRET: &'static str = "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";

    pub async fn from_code(redirect_url: String, code: String) -> Self {
        println!("{}, {:?}", redirect_url, redirect_url);
        let client = GoogleClient::new(
            ClientId::new(Self::CLIENT_ID.into()),
            Some(ClientSecret::new(Self::CLIENT_SECRET.into())),
            AUTH_URL.clone(),
            Some(TOKEN_URL.clone()),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_url).expect("Failed to parse redirect_url"));

        let auth = client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .expect("Failed to get auth from code");

        assert_eq!(*auth.token_type(), BasicTokenType::Bearer);

        let expires_at = auth.expires_in().map(|duration| Instant::now() + duration);
        let refresh_token = auth.refresh_token().map(|token| token.clone());

        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.insecure_disable_signature_validation();
        validation.validate_aud = false;
        validation.validate_exp = false;

        let id_token_decoded = jsonwebtoken::decode::<IdTokenDecoded>(
            &auth.extra_fields().id_token,
            &jsonwebtoken::DecodingKey::from_secret(&[]),
            &validation,
        )
        .expect("With verification disabled this is infallible");

        GoogleSession {
            bearer_access_token: auth.access_token().clone(),
            expires_at_and_refresh_token: match (expires_at, refresh_token) {
                (None, None) => None,
                (None, Some(_)) => {
                    // Access token never expires so we don't need the refresh token
                    None
                },
                (Some(_), None) => panic!("Only expires_at given, no refresh token given. This will expire at some point and log out the user"),
                (Some(expires_at), Some(refresh_token)) => Some((expires_at, refresh_token)),
            },
            sub: id_token_decoded.claims.sub,
        }
    }

    pub async fn bearer_header(&mut self) -> reqwest::header::HeaderValue {
        todo!()
    }
}
