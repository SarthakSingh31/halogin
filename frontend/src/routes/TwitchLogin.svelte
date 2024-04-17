<script lang="ts">
    import { Button } from "flowbite-svelte";

    export let keepLoggedIn: Boolean = false;
    export let onSuccess: (resp: Response) => void = () => {};

    function login() {
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
                        keep_logged_in: keepLoggedIn,
                    }),
                })
                    .then(onSuccess)
                    .catch(console.error);
            } else {
                console.error("Wrong state on twitch login response");
            }
        };

        window.open(
            `https://id.twitch.tv/oauth2/authorize?response_type=code&force_verify=true&client_id=${TWITCH_CLIENT_ID}&redirect_uri=${REDIRECT_URI}&scope=${TWITCH_SCOPES}&state=${STATE}`,
            "_blank",
            "height=600,width=400",
        );
    }
</script>

<Button on:click={login}>Connect with Twitch</Button>
