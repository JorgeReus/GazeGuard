# Windows Desktop Signals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Windows desktop signal provider that reports native idle state and detects when another foreground app is fullscreen, while preserving heuristic fallback behavior.

**Architecture:** Refactor `src-tauri/src/desktop_signals.rs` around a small provider trait implemented per platform. Add a Windows provider behind `#[cfg(all(desktop, target_os = "windows"))]` that uses Win32 APIs for last-input timing and foreground-window monitor coverage, then merges those native signals with the existing fallback snapshot.

**Tech Stack:** Rust, Tauri 2, Win32 APIs via `windows-sys`, cargo test

---

### Task 1: Introduce Provider-Shaped Windows Signal Helpers

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Add failing helper tests for Windows signal composition and fullscreen exclusion**
- [ ] **Step 2: Run `cd src-tauri && cargo test windows_signals_from_sources_uses_native_values_when_present` and verify it fails**
- [ ] **Step 3: Add helper types and functions that compose fallback/native Windows signals and decide whether a foreground window should count as another app fullscreen**
- [ ] **Step 4: Run `cd src-tauri && cargo test windows_signals_from_sources_uses_native_values_when_present` and verify it passes**

### Task 2: Convert Platform Selection To A Provider Trait

**Files:**
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Add failing tests or compile checks for the refactored provider path**
- [ ] **Step 2: Run `cd src-tauri && cargo test` and observe the failure or compile error**
- [ ] **Step 3: Introduce the provider trait and update the existing macOS, Linux, and fallback modules to implement it without changing current behavior**
- [ ] **Step 4: Run `cd src-tauri && cargo test` and verify existing tests still pass**

### Task 3: Add Native Windows Idle And Fullscreen Collection

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/desktop_signals.rs`
- Test: `src-tauri/src/desktop_signals.rs`

- [ ] **Step 1: Add failing tests for Windows fullscreen geometry helpers**
- [ ] **Step 2: Run `cd src-tauri && cargo test windows_fullscreen_helper_detects_foreground_window_covering_monitor` and verify it fails**
- [ ] **Step 3: Add `windows-sys` target dependencies and implement the Windows provider using Win32 last-input, foreground-window, and monitor-bound queries**
- [ ] **Step 4: Run `cd src-tauri && cargo test` and verify the full suite passes**
