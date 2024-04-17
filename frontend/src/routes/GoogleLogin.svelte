<script lang="ts">
    import { Button } from "flowbite-svelte";
    import { GoogleOAuthProvider } from "google-oauth-gsi";

    export let keepLoggedIn: Boolean = false;
    export let onSuccess: (resp: Response) => void = () => {};

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
                        keep_logged_in: keepLoggedIn,
                    }),
                })
                    .then(onSuccess)
                    .catch(console.error);
            } else {
                console.error("No auth code recieved from google auth");
            }
        },
        onError: (err) => console.error("Failed to login with google", err),
    });
</script>

<Button on:click={() => login()}>Sign in with Google</Button>
