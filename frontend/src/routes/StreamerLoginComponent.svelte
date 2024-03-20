<script lang="ts">
    import { Checkbox, DarkMode, Heading } from "flowbite-svelte";
    import {
        Navbar,
        NavBrand,
        NavLi,
        NavUl,
        NavHamburger,
        Button,
        Input,
        GradientButton,
    } from "flowbite-svelte";
    import { SearchOutline } from "flowbite-svelte-icons";
    import Logo from "../lib/logo.svg";

    let checked = false;

    function signInCallback(authResult: { code: string }) {
        if (authResult["code"]) {
            fetch("login/google", {
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
            });
        } else {
            // There was an error.
        }
    }

    function googleSignInCallback() {
        // @ts-expect-error
        auth2.grantOfflineAccess().then(signInCallback);
    }
</script>

<svelte:head>
    <script src="//ajax.googleapis.com/ajax/libs/jquery/1.8.2/jquery.min.js">
    </script>
    <script
        src="https://apis.google.com/js/client:platform.js?onload=start"
        async
        defer
    >
    </script>
    <script>
        function start() {
            gapi.load("auth2", function () {
                auth2 = gapi.auth2.init({
                    client_id:
                        "751704262503-61e56pavvl5d8l5fg6s62iejm8ft16ac.apps.googleusercontent.com",
                    // Scopes to request in addition to 'profile' and 'email'
                    //scope: 'additional_scope'
                    plugin_name: "halogin",
                });
            });
        }
    </script>
</svelte:head>

<div>
    <button on:click={googleSignInCallback}>Sign in with Google</button>
    <button id="twitchSigninButton">Connect with Twitch</button>
    <Checkbox bind:checked>Keep logged in</Checkbox>
    <script>
        $("#twitchSigninButton").click(function () {
            const CLIENT_ID = "65x8qdhtinpz5889thff2ae4o0nxrw";
            window.open(
                `https://id.twitch.tv/oauth2/authorize?response_type=code&client_id=${CLIENT_ID}&redirect_uri=${window.location.origin}&scope=channel%3Amanage%3Apolls+channel%3Aread%3Apolls&state=c3ab8aa609ea11e793ae92361f002671`,
            );
        });
    </script>
</div>
