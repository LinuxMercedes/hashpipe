## 0.1.0: Initial Release
Hashpipe was rudely released upon an unsuspecting world.

### 0.1.1: Connection bugfix
When piping input in in raw-input mode, hashpipe was too eager to send its stdin to the server. Now it patiently waits for the server to listen before speaking.

### 0.1.2: Bump `clap` version
The current version of `clap` would not build on new versions of `rustc`.