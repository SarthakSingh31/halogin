<script lang="ts">
    import { Checkbox } from "flowbite-svelte";
    import { Button } from "flowbite-svelte";
    import { GoogleOAuthProvider } from "google-oauth-gsi";

    let checked = false;

    const googleProvider = new GoogleOAuthProvider({
        clientId:
            "751704262503-61e56pavvl5d8l5fg6s62iejm8ft16ac.apps.googleusercontent.com",
        onScriptLoadError: () => console.log("onScriptLoadError"),
        onScriptLoadSuccess: () => console.log("onScriptLoadSuccess"),
    });
    const login = googleProvider.useGoogleLogin({
        flow: "auth-code",
        scope: "https://www.googleapis.com/auth/youtube.readonly https://www.googleapis.com/auth/youtube.channel-memberships.creator https://www.googleapis.com/auth/yt-analytics.readonly",
        onSuccess: (authResult: { code: string }) => {
            if (authResult["code"]) {
                fetch("/api/v1/google/login", {
                    method: "POST",
                    headers: {
                        "X-Requested-With": "XMLHttpRequest",
                        "Content-Type": "application/json; charset=utf-8",
                    },
                    body: JSON.stringify({
                        redirect_origin: window.location.origin,
                        code: authResult["code"],
                        keep_logged_in: checked,
                    }),
                }).catch(console.error);
            } else {
                console.error("No auth code recieved from google auth");
            }
        },
        onError: (err) => console.error("Failed to login with google", err),
    });

    function twitchLogin() {
        const TWITCH_CLIENT_ID = "65x8qdhtinpz5889thff2ae4o0nxrw";
        const TWITCH_SCOPES = encodeURIComponent(
            ["channel:read:subscriptions", "moderator:read:followers"].join(
                " ",
            ),
        );
        const REDIRECT_URI = encodeURIComponent(
            `${window.location.origin}/login_redirect`,
        );
        const STATE =
            Math.random().toString(36).slice(2) +
            Math.random().toString(36).slice(2) +
            Math.random().toString(36).slice(2);

        window.twitchCallback = (state: string, code: string) => {
            if (state == STATE) {
                fetch("/api/v1/twitch/login", {
                    method: "POST",
                    headers: {
                        "X-Requested-With": "XMLHttpRequest",
                        "Content-Type": "application/json; charset=utf-8",
                    },
                    body: JSON.stringify({
                        redirect_origin: decodeURIComponent(REDIRECT_URI),
                        code,
                        keep_logged_in: checked,
                    }),
                }).catch(console.error);
            } else {
                console.error("Wrong state on twitch login response");
            }
        };

        window.open(
            `https://id.twitch.tv/oauth2/authorize?response_type=code&client_id=${TWITCH_CLIENT_ID}&redirect_uri=${REDIRECT_URI}&scope=${TWITCH_SCOPES}&state=${STATE}`,
            "_blank",
            "height=600,width=400",
        );
    }
</script>

<div>
    <Button on:click={() => login()}>Sign in with Google</Button>
    <Button on:click={twitchLogin}>Connect with Twitch</Button>
    <Checkbox bind:checked>Keep logged in</Checkbox>
</div>
