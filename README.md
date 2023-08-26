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
├── Announcements
├── Assessment
│   ├── Assignment One: Due Week 6
│   └── Quiz Solutions
│       ├── Quiz 1 Solutions
│       ├── Quiz 2 Solutions
│       └── Quiz 3 Solutions
├── Course Help
│   └── Student services and resources
├── Course Profile (ECP)
├── Course Staff
├── Ed Discussion
├── Gradescope
├── Learning Resources
│   ├── Course Reading List
│   ├── Lecture_Recordings
│   ├── Subject Guides
│   └── Transcripts – Advice
├── Library Links
└── My Grades

5 directories, 16 files
```

As a bonus, browsing the filesystem is significantly faster than browsing the Blackboard web UI,
which is very helpful when you're stuck on slow campus WiFi.

