// Public client id of PCL2-CE (extracted from its release binary and re-validated
// against the consumers device-code endpoint, see .owv/n3-client-id-check.md).
// PCL2-CE's own id is used on purpose: it is the client the WTUMC skin site and
// the PCL lineage already work with. Our own registration is still pending, so
// this one stays the default; a build can inject another with MS_CLIENT_ID.
pub static CLIENT_ID: &str = match option_env!("MS_CLIENT_ID") {
  Some(client_id) => client_id,
  None => "c14b0370-8d75-42f8-b329-5b60d39e319f",
};
pub static SCOPE: &str = "XboxLive.signin offline_access";
pub static DEVICE_AUTH_ENDPOINT: &str =
  "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
pub static FRIENDS_ENDPOINT: &str = "https://api.minecraftservices.com/friends";
pub static PRESENCE_ENDPOINT: &str = "https://api.minecraftservices.com/presence";
pub static PROFILE_ENDPOINT: &str = "https://api.minecraftservices.com/minecraft/profile";
pub static XSTS_AUTH_ENDPOINT: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub static MINECRAFT_TOKEN_ENDPOINT: &str =
  "https://api.minecraftservices.com/authentication/login_with_xbox";
pub static OAUTH_TOKEN_ENDPOINT: &str =
  "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
