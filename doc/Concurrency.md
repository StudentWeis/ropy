**Concurrency Analysis**

This document details the threading model and message passing architecture of the Ropy application.

# Thread Model

The application uses a hybrid architecture combining dedicated OS threads for blocking operations and GPUI's async executors for event handling and background processing.

| Thread | Responsibility | Spawned By |
| :--- | :--- | :--- |
| **Main Thread** | Runs the GPUI event loop, handles UI rendering, processes events, and runs the Hotkey Listener task. | OS / `main` |
| **Clipboard Watcher** | Uses `clipboard-rs` to monitor system clipboard changes. | `clipboard::start_clipboard_monitor` |
| **Clipboard Writer** | Handles requests to write text or images back to the system clipboard. | `clipboard::start_clipboard_writer` |
| **Tray Listener** | Listens for system tray menu events (Show, Quit). | `tray::init_tray` |
| **GPUI Background** | A thread pool managed by GPUI for running async tasks (Image Processing, Clipboard Event Handling). | GPUI Runtime |
| **RSS Monitor** | (Debug only) Monitors RSS usage. | `monitor::spawn_rss_monitor` |

> **Note**: The "Image Processor" and "Hotkey Listener" are implemented as async tasks running on the GPUI executors rather than dedicated OS threads.

# Message Passing

The application relies on channels (`std::sync::mpsc` and `async_channel`) for communication between threads and async tasks.

## 1. Clipboard Monitoring Flow

- **Source**: `Clipboard Watcher` thread detects a change.
- **Path 1 (Text)**: Sends `ClipboardEvent::Text` via `clipboard_tx` (async_channel) to the **Clipboard Listener Task**.
- **Path 2 (Image)**:
  1. Sends `DynamicImage` via `image_tx` (async_channel) to the **Image Processor Task** (running on GPUI Background executor).
  2. **Image Processor Task** saves the image and sends `ClipboardEvent::Image` via `clipboard_tx` to the **Clipboard Listener Task**.
- **Handling**: The **Clipboard Listener Task** (running on GPUI Background executor) receives `ClipboardEvent`, updates the `Repository`, and updates the `SharedRecords`.

## 2. Hotkey Flow

- **Source**: `GlobalHotKeyEvent` receiver.
- **Mechanism**: A task on the **Main Thread** (Foreground Executor) polls the receiver.
- **Handling**: When a hotkey is detected, the task directly dispatches an `Active` action to the window via `async_app.update`.

## 3. Tray Flow

- **Source**: `Tray Listener` thread detects menu click.
- **Path**: Sends `TrayEvent` via `tray_tx` (mpsc) to the **Tray Handler Task**.
- **Handling**: The **Tray Handler Task** (running on Main Thread / Foreground Executor) polls `tray_rx` and either shows the window or quits the app.

## 4. Copy/Paste Flow

- **Source**: User interaction in **Main App** (UI).
- **Path**: Sends `CopyRequest` via `copy_tx` (mpsc) to the `Clipboard Writer` thread.
- **Handling**: `Clipboard Writer` writes the content to the system clipboard.

# Architecture Diagram

```mermaid
graph TD
    subgraph "Background Threads"
        CW[Clipboard Watcher]
        CWr[Clipboard Writer]
        TL[Tray Listener]
    end

    subgraph "GPUI Runtime (Main Thread)"
        Main[Main App / UI]
        HL[Hotkey Listener Task]
        TH[Tray Handler Task]
    end

    subgraph "GPUI Runtime (Background Pool)"
        IP[Image Processor Task]
        CL[Clipboard Listener Task]
    end

    subgraph "Data"
        Repo[Repository]
        Shared[Shared Records]
    end

    %% Clipboard Monitoring
    CW -- "Text Event" --> CL
    CW -- "DynamicImage" --> IP
    IP -- "Image Event" --> CL
    CL -- "Save/Update" --> Repo
    CL -- "Update" --> Shared

    %% User Input
    HL -- "Dispatch Action" --> Main
    TL -- "Tray Event" --> TH
    TH -- "Update/Quit" --> Main

    %% Clipboard Writing
    Main -- "CopyRequest" --> CWr
    CWr -- "Write" --> SystemClipboard((System Clipboard))
    CW -. "Watch" .-> SystemClipboard
```

# Detailed Data Flow

## Clipboard Event Processing

```mermaid
sequenceDiagram
    participant Sys as System Clipboard
    participant CW as Clipboard Watcher
    participant IP as Image Processor (Task)
    participant CL as Clipboard Listener (Task)
    participant Repo as Repository
    participant Shared as Shared Records

    Sys->>CW: Content Changed
    alt is Text
        CW->>CL: ClipboardEvent::Text
    else is Image
        CW->>IP: DynamicImage
        IP->>IP: Save to Disk
        IP->>CL: ClipboardEvent::Image(Path)
    end

    activate CL
    CL->>Repo: Save Record
    Repo-->>CL: Record
    CL->>Shared: Update In-Memory List
    deactivate CL
```
