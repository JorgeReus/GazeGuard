import App from './App.svelte';
import '@wdio/tauri-plugin';
import './styles.css';
import { mount } from 'svelte';

mount(App, { target: document.getElementById('app')! });
