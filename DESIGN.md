# Launcher-MC Design Specification

## Visual direction

Launcher-MC uses the supplied concept direction as the visual baseline:

- Dark theme: near-black surfaces, subtle borders, Minecraft-inspired green accent.
- Light theme: warm/light surfaces with the same layout and accent system.
- Optional accent color is centralized so future custom themes do not require component rewrites.
- Sidebar-first desktop layout with Home, Instances, Mods, Resource Packs, Worlds, Servers, Accounts, and Settings.
- Home has a large selected-instance hero, primary Play action, recent instances, and launcher news/activity.
- Dense management screens use cards, tables, filters, searchable lists, and contextual actions.
- Motion should be restrained: quick transitions, progress feedback, and meaningful state changes only.

## Core feature set

### Instance and runtime
- Multi-Instance Isolation
- Automated Mod Dependency Resolution
- Granular Java Runtime Management
- Custom JVM Argument Profiles
- One-Click Snapshot/Alpha Rollback
- Offline Mode Fallback
- Cross-Platform Sync
- Texture Pack Version Control

### Operations and diagnostics
- Real-Time Performance Telemetry
- Crash Log Auto-Analysis
- Server Whitelist Integration
- Account Activity Logging

### Accounts and security
- Bulk Account Management
- Secure Credential Vault
- Microsoft OAuth 2.0 Integration
- Offline Yggdrasil Authentication
- Multi-Account Switching
- Token Expiration Auto-Renewal
- Local Profile Encryption
- Skin & Cape Management

### Access control
- Role-Based Access Control

## Architecture target

Tauri 2 + Rust backend + Svelte 5 + TypeScript frontend.

Rust owns privileged operations: filesystem access, process management, downloads, instance isolation, Java detection, Minecraft metadata, authentication token storage, encryption, crash analysis, and telemetry collection.

The Svelte frontend owns presentation and user interaction. Tauri commands/events form the explicit boundary between UI and native operations.

## Security requirements

- OAuth tokens and Microsoft credentials must never be persisted in plaintext.
- Secrets belong in the OS credential/keyring layer where available; encrypted local storage is the fallback.
- Renderer code must not receive long-lived refresh tokens unless strictly required by the authentication flow.
- Offline authentication must be explicitly labeled as offline/local and must never imply official Mojang/Microsoft authentication.
- RBAC is enforced in the Rust command layer, not only hidden in the UI.
- Telemetry is opt-in and must have a clear local-only mode.
