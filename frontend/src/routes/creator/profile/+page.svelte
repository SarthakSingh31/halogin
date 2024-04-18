<script lang="ts">
    import {
        Label,
        Input,
        Textarea,
        Dropzone,
        Fileupload,
        Avatar,
        Button,
    } from "flowbite-svelte";
    import default_img from "$lib/default.jpg";
    import { onMount } from "svelte";
    import CreatorCard from "../creator-card.svelte";
    import GoogleLogin from "../../GoogleLogin.svelte";
    import TwitchLogin from "../../TwitchLogin.svelte";

    let youtubeChannels: YoutubeChannel[] = [];
    let twitchAccounts: TwitchAccount[] = [];

    $: youtubePfpImages = youtubeChannels.map((channel) => {
        return {
            url: channel.snippet.thumbnails.high.url,
            file: null,
            isRemote: true,
        };
    });
    $: twitchPfpImages = twitchAccounts.map((account) => {
        return {
            url: account.profile_image_url,
            file: null,
            isRemote: true,
        };
    });

    onMount(() => {
        fetch("/api/v1/google/youtube/channel")
            .then((resp) => resp.json())
            .then((channels) => {
                youtubeChannels = channels;
            });
        fetch("/api/v1/twitch/account")
            .then((resp) => resp.json())
            .then((accounts) => {
                twitchAccounts = accounts;
            });
    });

    let selected = 0;

    $: selected_image = images[selected];
    $: remote_url = selected_image.isRemote ? selected_image.url : null;

    let files: any;
    let file_images: { url: string; file: File | null; isRemote: Boolean }[] =
        [];
    let last_file_url = "";

    $: {
        if (files && files[0]) {
            let reader = new FileReader();
            reader.onload = (evt) => {
                let new_file_url = evt.target?.result as string;
                if (last_file_url !== new_file_url) {
                    file_images = [
                        { url: new_file_url, file: files[0], isRemote: false },
                        ...file_images,
                    ];
                    selected = 0;
                    last_file_url = new_file_url;
                }
            };
            reader.readAsDataURL(files[0]);
        }
    }

    $: images = [
        ...file_images,
        ...youtubePfpImages,
        ...twitchPfpImages,
        { url: default_img, file: null, isRemote: false },
    ];

    function googleLoginSuccess(data: YoutubeChannel[]) {
        data.forEach((channelData) => {
            let hasThisAccount = false;
            youtubeChannels.forEach((channel) => {
                if (channel.id === channelData.id) {
                    hasThisAccount = true;
                }
            });

            if (!hasThisAccount) {
                youtubeChannels = [...youtubeChannels, channelData];
            }
        });
    }

    function twitchLoginSuccess(data: TwitchAccount) {
        let hasThisAccount = false;
        twitchAccounts.forEach((channel) => {
            if (channel.display_name === data.display_name) {
                hasThisAccount = true;
            }
        });

        if (!hasThisAccount) {
            twitchAccounts = [...twitchAccounts, data];
        }
    }

    let givenName = "";
    let familyName = "";
    let pronouns = "";

    let profileDescValue = "";
    let contentDescValue = "";
    let audienceDescValue = "";
</script>

<div class="grid md:grid-cols-2">
    <div class="min-h-screen max-h-screen overflow-x-scroll">
        <form
            class="m-8"
            action="/api/v1/creator/data"
            method="post"
            enctype="multipart/form-data"
        >
            <div class="grid gap-x-6 gap-y-2 mb-4 md:grid-cols-2">
                <div>
                    <Label for="given_name">Given name</Label>
                    <Input
                        type="text"
                        id="given_name"
                        name="given_name"
                        placeholder="John"
                        bind:value={givenName}
                        required
                    />
                </div>
                <div>
                    <Label for="family_name">Family name</Label>
                    <Input
                        type="text"
                        id="family_name"
                        name="family_name"
                        placeholder="Doe"
                        bind:value={familyName}
                        required
                    />
                </div>
                <div>
                    <Label for="pronouns">Pronouns</Label>
                    <Input
                        type="text"
                        id="pronouns"
                        name="pronouns"
                        placeholder="They/Them"
                        bind:value={pronouns}
                        required
                    />
                </div>
            </div>
            <div class="grid gap-6 mb-4 md:grid-cols-2">
                <div>
                    {#each youtubeChannels as channel}
                        <Button
                            on:click={() => {
                                window.open(
                                    `https://www.youtube.com/${channel.snippet.customUrl}`,
                                );
                            }}
                        >
                            <Avatar
                                size="xs"
                                src={channel.snippet.thumbnails.high.url}
                                border
                            />
                            <span class="ml-2">{channel.snippet.title}</span>
                        </Button>
                    {/each}
                    <br />
                    <GoogleLogin
                        onSuccess={(resp) =>
                            resp.json().then(googleLoginSuccess)}
                    />
                </div>
                <div>
                    {#each twitchAccounts as account}
                        <Button
                            on:click={() => {
                                window.open(
                                    `https://www.twitch.tv/${account.display_name}`,
                                );
                            }}
                        >
                            <Avatar
                                size="xs"
                                src={account.profile_image_url}
                                border
                            />
                            <span class="ml-2">{account.display_name}</span>
                        </Button>
                    {/each}
                    <br />
                    <TwitchLogin
                        onSuccess={(resp) =>
                            resp.json().then(twitchLoginSuccess)}
                    />
                </div>
            </div>
            <div class="grid gap-6 mb-4 md:grid-cols-2">
                <div>
                    <Label for="pfp" class="mb-2">Upload Profile Picture</Label>
                    <Fileupload
                        id="pfp"
                        name="pfp"
                        accept="image/*"
                        bind:files
                    />
                    <div class="my-4 overflow-x-scroll whitespace-nowrap">
                        {#each images as image, i}
                            <button
                                class="{i !== images.length - 1
                                    ? 'pr-2'
                                    : ''} inline-block"
                                on:click={(evt) => {
                                    evt.preventDefault();
                                    selected = i;
                                }}
                            >
                                <img
                                    src={image.url}
                                    alt="You"
                                    class="image-queue"
                                />
                            </button>
                        {/each}
                    </div>
                    <input type="hidden" name="pfp_hidden" value={remote_url} />
                </div>
                <div>
                    <img
                        id="pfp-img"
                        src={selected_image.url}
                        alt="You"
                        class="m-auto"
                    />
                </div>
            </div>
            <div class="mb-4">
                <Label for="profile_desc" class="mb-2">
                    Write a small introduction of yourself
                </Label>
                <Textarea
                    id="profile_desc"
                    name="profile_desc"
                    placeholder="My name is John Doe. I am a child friendly youtuber."
                    rows="7"
                    bind:value={profileDescValue}
                    required
                />
            </div>
            <div class="mb-4">
                <Label for="content_desc" class="mb-2">
                    Write a detailed description of your content
                </Label>
                <Textarea
                    id="content_desc"
                    name="content_desc"
                    placeholder="I make videos about video games."
                    rows="7"
                    bind:value={contentDescValue}
                    required
                />
            </div>
            <div class="mb-4">
                <Label for="audience_desc" class="mb-2">
                    Write a description of your audience
                </Label>
                <Textarea
                    id="audience_desc"
                    name="audience_desc"
                    placeholder="My audience mostly consists of video game nerds."
                    rows="7"
                    bind:value={audienceDescValue}
                    required
                />
            </div>
            <div>
                <input
                    type="submit"
                    value="Submit"
                    class="m-auto text-white hover:bg-primary-800 bg-primary-600 border-primary-800 border-2 block py-2 px-8 rounded-lg"
                />
            </div>
        </form>
    </div>
    <div class="min-h-screen max-h-screen overflow-x-scroll">
        <CreatorCard
            class="m-6"
            data={{
                givenName,
                familyName,
                pronouns,
                pfpImageUrl: selected_image.url,
                profileDesc: profileDescValue,
                contentDesc: contentDescValue,
                audienceDesc: audienceDescValue,
            }}
            defaults={{
                givenName: "John",
                familyName: "Doe",
                pronouns: "They/Them",
                profileDesc:
                    "My name is John Doe. I am a child friendly youtuber.",
                contentDesc: "I make videos about video games.",
                audienceDesc:
                    "My audience mostly consists of video game nerds.",
            }}
            {youtubeChannels}
            {twitchAccounts}
        />
    </div>
</div>

<style lang="scss">
    #pfp-img {
        max-height: 200px;
    }
    .image-queue {
        max-height: 100px;
    }
</style>
