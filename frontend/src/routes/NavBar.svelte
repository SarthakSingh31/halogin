<script>
    import {
        Navbar,
        NavBrand,
        NavLi,
        NavUl,
        NavHamburger,
        Button,
        Modal,
    } from "flowbite-svelte";
    import StreamerLoginComponent from "./StreamerLoginComponent.svelte";
    import Logo from "../lib/logo.svg";

    let open = false;
</script>

<div class="relative px-8">
    <Navbar
        class="px-2 sm:px-4 py-2.5 fixed w-full z-20 top-0 start-0 border-b"
    >
        <NavBrand href="/">
            <img src={Logo} class="me-3 h-6 sm:h-9" alt="Flowbite Logo" />
            <span
                class="self-center whitespace-nowrap text-xl font-semibold dark:text-white"
                >Halogin</span
            >
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
    <Modal bind:open><StreamerLoginComponent /></Modal>
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
