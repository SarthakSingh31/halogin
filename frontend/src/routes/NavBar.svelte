<script>
    import { Navbar, NavBrand, NavLi, NavUl, NavHamburger, ImagePlaceholder, Skeleton, TextPlaceholder, Button } from 'flowbite-svelte';
    import Logo from "../lib/logo.svg";
    import GoogleLogin from "./GoogleLogin.svelte";
    import TwitchLogin from "./TwitchLogin.svelte";

    let open = false;
    let keepLoggedIn = false;
</script>

<style>
    .color{
       background-color: #ecf0f3;
    }
    .nav-item {
        padding-top: 10px;
        padding-bottom: 10px; 
        margin-bottom: 8px; 
        display: flex;
        align-items: center;
    }
    .button-container {
        display: flex;
        align-items: center;
        gap: 8px; 
    }
</style>

<div class="relative px-8 ">
    <Navbar class="px-2 sm:px-4 py-2.5 fixed w-full z-20 top-0 start-0 border-b">
        <NavBrand href="/">
            <img src={Logo} class="me-3 h-6 sm:h-9" alt="Flowbite Logo" />
            <span
                class="self-center whitespace-nowrap text-xl font-semibold dark:text-white"
            >
                Halogin
            </span>
        </NavBrand>
        <NavHamburger />
        <NavUl>
            <div class="nav-item">
                <NavLi href="/" active={true}>Find Deals</NavLi>
            </div>
            <div class="nav-item">
                <NavLi href="/">Find Creators</NavLi>
            </div>
            <div class="nav-item">
                <NavLi href="/">About Us</NavLi>
            </div>
            <div class="button-container">
                <Button
                    size="xl"
                    color="green"
                    class="nav-item"
                    on:click={() => {
                        open = true;
                    }}
                >
                    Log In
                </Button>
                <Button size="xl" color="green" class="nav-item">Sign Up</Button
                >
            </div>
        </NavUl>
    </Navbar>
    <Modal bind:open>
        <GoogleLogin
            bind:keepLoggedIn
            onSuccess={() => (window.location.pathname = "creator/profile")}
        />
        <TwitchLogin
            bind:keepLoggedIn
            onSuccess={() => (window.location.pathname = "creator/profile")}
        />
        <Checkbox bind:checked={keepLoggedIn}>Keep logged in</Checkbox>
    </Modal>
</div>

<style>
    .nav-item {
        padding-top: 10px;
        padding-bottom: 10px;
        margin-bottom: 8px;
        display: flex;
        align-items: center;
    }
    .button-container {
        display: flex;
        align-items: center;
        gap: 8px;
    }
</style>
