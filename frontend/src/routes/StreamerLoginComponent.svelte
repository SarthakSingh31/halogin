<script lang="ts">
    import { DarkMode, Heading } from "flowbite-svelte";
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
    <button id="googleSigninButton">Sign in with Google</button>
    <button id="twitchSigninButton">Connect with Twitch</button>
    <script>
        $("#googleSigninButton").click(function () {
            auth2.grantOfflineAccess().then(signInCallback);
        });
        $("#twitchSigninButton").click(function () {
            auth2.grantOfflineAccess().then(signInCallback);
        });
    </script>
    <script>
        function signInCallback(authResult) {
            console.log(authResult);

            if (authResult["code"]) {
                // Hide the sign-in button now that the user is authorized, for example:
                $("#signinButton").attr("style", "display: none");

                // Send the code to the server
                $.ajax({
                    type: "POST",
                    url: "login/google",
                    // Always include an `X-Requested-With` header in every AJAX request,
                    // to protect against CSRF attacks.
                    headers: {
                        "X-Requested-With": "XMLHttpRequest",
                    },
                    contentType: "application/json; charset=utf-8",
                    success: function (result) {
                        // Handle or verify the server response.
                    },
                    processData: false,
                    data: JSON.stringify({
                        redirect_origin: window.location.origin,
                        code: authResult["code"],
                    }),
                });
            } else {
                // There was an error.
            }
        }
    </script>
</div>
