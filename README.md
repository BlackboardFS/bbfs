# BlackboardFS

> Blackboard: *noun* A website so bad that it might as well be a network drive.

BlackboardFS is a filesystem driver that allows you to view your Blackboard course contents as if
they were normal files and folders on your system!

![A banner image demonstrating how BlackboardFS maps Blackboard courses to folders](
  static/banner.png
  "Max, you forgot the red circle to go along with the arrow!"
)

The filesystem matches Blackboard's structure as closely as possible, with the same familiar sidebar
names and even the course's internal folder structure!

```
$ tree Blackboard/COMP3506
Blackboard/COMP3506
├── Announcements.desktop
├── Assessment
│   ├── Assignment One: Due Week 6
│   ├── Blackboard.desktop
│   └── Quiz Solutions
│       ├── Blackboard.desktop
│       ├── quiz1-sol.pdf
│       ├── quiz2-sol%281%29.pdf
│       └── quiz3-sol.pdf
├── Blackboard.desktop
├── Course Help
│   ├── Blackboard.desktop
│   └── Student services and resources
├── Course Profile (ECP).desktop
├── Course Staff.desktop
├── Ed Discussion.desktop
├── Gradescope.desktop
├── Learning Resources
│   ├── Blackboard.desktop
│   ├── Code Snippets
│   │   ├── Blackboard.desktop
│   │   ├── Week 1
│   │   ├── Week 2
│   │   ├── Week 3
│   │   └── Week 4
│   ├── COMP3506-7505-2023-plan-v3.pdf
│   ├── Course Reading List.desktop
│   ├── Lecture_Recordings.desktop
│   ├── Resources

--snip--

15 directories, 70 files
```

Links to external resources are exposed as `.url` (Windows), `.webloc` (macOS) or `.desktop` (Linux)
files, so you can easily reach Gradescope, echo360, and even get back to Blackboard's own web UI
right from your file browser!

As a bonus, browsing the filesystem is significantly faster than browsing the Blackboard web UI,
which is very helpful when you're stuck on slow campus WiFi.

## Requirements

To build from source, the latest stable [Rust](https://rustup.rs/) toolchain must be installed.
Other platform-specific runtime dependencies are described below:

### Windows

BlackboardFS requires [Dokan](https://dokan-dev.github.io/) be installed on your system.

### macOS

BlackboardFS requires [macFUSE](https://osxfuse.github.io/) be installed on your system.

### Linux

The auth window requires GTK3, WebKitGTK, and related libraries be installed on your system.
Additionally, to mount the filesystem, FUSE3 is required.
Make sure the following packages are installed:

#### Debian/Ubuntu

```
sudo apt install libwebkit2gtk-4.1-dev libfuse3-dev
```

#### Fedora

```
sudo dnf install gtk3-devel webkit2gtk4.1-devel fuse3-devel
```

#### Arch/Manjaro

```
sudo pacman -S webkit2gtk-4.1 fuse3
```

## Installation
Clone this repo to a location of your choosing. Then
```
git submodule update && git submodule init
```
You can then `cargo run -p bbfs-cli` or `cargo install --bin bbfs-cli` as you wish.

## Usage

This is a FUSE-based filesystem. To mount:

```
bbfs <mount_point>
```

This will spawn a browser window for you to log in with your UQ login. **WE ARE ABLE TO INJECT
ARBITRARY CODE INTO THIS BROWSER WINDOW, SO MAKE SURE YOU READ AND UNDERSTAND OUR CODE TO MAKE SURE
WE'RE NOT STEALING YOUR CREDENTIALS** (the relevant code is in `cookie_monster/`; everything else
only gets a session token).

To unmount the filesystem:

```
fusermount -u <mount_point>
```
or on MacOS:
```
diskutil unmount <mount_point>
```
