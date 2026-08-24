import Break from './Break.svelte';
import '@wdio/tauri-plugin';
import { mount } from 'svelte';

mount(Break, { target: document.getElementById('app')! });
