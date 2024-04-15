<script lang="ts">
    import {
        Label,
        Input,
        Textarea,
        Dropzone,
        Fileupload,
    } from "flowbite-svelte";
    import default_img from "$lib/default.jpg";
    import { onMount } from "svelte";
    import CreatorCard from "../creator-card.svelte";

    let files: any;
    let images: { url: string; file: File | null; isRemote: Boolean }[] = [
        { url: default_img, file: null, isRemote: false },
    ];
    let selected = 0;
    let last_url = "";

    $: {
        if (files && files[0]) {
            let reader = new FileReader();
            reader.onload = (evt) => {
                let new_url = evt.target?.result as string;
                if (last_url !== new_url) {
                    images = [
                        { url: new_url, file: files[0], isRemote: false },
                        ...images,
                    ];
                    selected = 0;
                    last_url = new_url;
                }
            };
            reader.readAsDataURL(files[0]);
        }
    }
    $: selected_image = images[selected];
    $: remote_url = selected_image.isRemote ? selected_image.url : null;

    onMount(() => {
        fetch("/api/v1/creator/account_pfps")
            .then((resp) => resp.json())
            .then((image_urls) =>
                image_urls.forEach((image_url: string) => {
                    images = [
                        { url: image_url, file: null, isRemote: true },
                        ...images,
                    ];
                }),
            );
    });

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
