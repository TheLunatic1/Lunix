# Automatic Google Drive Conversation Sync & Backup Rule

## Overview
This rule ensures that all active conversations, message transcripts, and project plans are safely synchronized with Google Drive without wasting cloud storage space.

## Guidelines
1. **Automated Sync**:
   - Whenever concluding a session, completing a major plan/walkthrough, or updating important project state, ensure the latest conversation history and text transcripts are backed up to Google Drive.
   - The backup is handled by the lightweight background sync script at `C:\Users\Salman Toha\.gemini\sync_to_gdrive.ps1`.
   
2. **Text & Transcript Priority**:
   - Only sync essential conversation data (`.db` SQLite history databases, `.jsonl` message transcripts, `.md` plans, and logs).
   - Never sync heavy media (`.png`, `.jpg`, `.mp4`, `.apk`, `.zip`) to Google Drive to keep cloud usage minimal and fast.

3. **Restoration & Recovery**:
   - If a conversation was closed abruptly, past transcripts and databases can be referenced from `G:\My Drive\AntigravitySync\` or `%USERPROFILE%\.gemini\antigravity-ide\conversations\`.
