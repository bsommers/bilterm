# Chapter 16: Retro Speech (`:say`)

Bilterm can talk. `:say` speaks text with an **old-school formant speech synthesizer** in the style
of 1980s hardware: SAM on 8-bit home computers, the Votrax arcade chip, DECtalk and the ring-modulated
computer voices of Tron. It is purely algorithmic: spelling is converted to phonemes by hand-written
letter-to-sound rules (no dictionary download), and the phonemes are turned into sound by a small
source-filter model (a buzzing voice source through resonant filters). There is no machine learning, no
model file, no network access and no voice data to install. The same text always sounds exactly the same.

## 1. Quick start

```text
:say hello world
:say --voice tron greetings program
:say --voice votrax --speed 1.4 --pitch 140 insert coin to continue
:say stop
```

From a shell (no running Bilterm needed, works over SSH):

```text
bilterm say --voice tron "Shall we play a game?"
bilterm say --wav hello.wav --voice sam "hello world"
echo "build finished" | bilterm say -
bilterm say --voices
```

## 2. Voices and options

| Voice | Character |
|-------|-----------|
| `robot` (default) | Flat buzzing robot with a light bit-crush |
| `tron` | Deep ring-modulated computer voice with a short echo |
| `votrax` | Choppy arcade speech chip |
| `sam` | 8-bit home-computer monotone at a low sample rate |
| `dectalk` | Smoother hardware synthesizer with lively pitch |

| Option | Meaning | Range |
|--------|---------|-------|
| `--voice NAME` (`-v`) | Voice preset | see above |
| `--speed X` (`-s`) | Speaking rate multiplier | 0.5 to 3.0 (default 1.0) |
| `--pitch HZ` (`-p`) | Base pitch | 50 to 300 Hz (default per voice) |
| `--wav NAME` (`-w`) | Write a WAV instead of playing | `:say`: a name `[A-Za-z0-9_-]{1,64}`, saved as `<runtime dir>/speech/NAME.wav` (mode 0600, directory 0700). `bilterm say`: any file path |

Text rules: numbers are spoken (`1984` is "one thousand nine hundred eighty four"), a few symbols are
spoken (`& % @ + = # / $`), `, ; :` give a short pause, `.` `!` `?` end a sentence (a question rises, a
statement falls), words without vowels such as `tcp` are spelled out, and anything else (control
characters, non-ASCII text) is treated as a word separator. Put `--` before text that starts with a
dash, for example `:say -- stop` speaks the word "stop".

## 3. Verbs

| Command | Effect |
|---------|--------|
| `:say <text>` | Speak (also `:speak`, `:tts`) |
| `:say stop` | Cancel the current utterance and the queue |
| `:say status` | State (`idle`, `rendering`, `speaking`), queue depth, audio player and the last error |
| `:say voices` | List the voice presets |

## 4. Limits and safety

- At most **2000 characters** per request and **30 seconds** of audio per utterance (longer text is cut off and reported).
- At most **4 utterances** queued; a fifth is refused with "speech queue full". `:say stop` clears the queue.
- Synthesis and playback run on a background thread. The UI never waits for audio.
- Playback shells out to the platform player: `paplay`, `pw-play`, `aplay`, `play` (sox) or `ffplay` on Linux, `afplay` on macOS, PowerShell `Media.SoundPlayer` on Windows. The temporary WAV is written privately (0600) and removed afterwards. If none is installed, `:say` reports it and `--wav` still works. Set `BILTERM_SAY_PLAYER=off` to disable audio output.
- Nothing is sent anywhere. The speech code makes no network calls.

## 5. Surfaces (`UNI-R01`)

| Capability | Colon | RPC | MCP (`bilterm_speech`) | CLI |
|------------|-------|-----|------------------------|-----|
| Speak / write WAV | `:say ...` | `speech.say` | `action: "say"` | `bilterm say ...` (in-process) |
| Stop | `:say stop` | `speech.stop` | `action: "stop"` | none (see below) |
| Status | `:say status` | `speech.status` | `action: "status"` | none (see below) |
| Voices | `:say voices` | `speech.voices` | `action: "voices"` | `bilterm say --voices` |

All of them lower to `Action::Speech` and run through `WindowManager::apply`. Over a socket use
`bilterm ctl speech.say '{"text":"hello","voice":"tron"}'`.

Deliberate exceptions: there is **no default key chord, menu entry or mouse gesture** (speech is an output action, not a
pane operation; bind one with a `[keys]` profile to `:say ...` if you want it), and `bilterm say` is
self-contained (it renders and plays inside its own process), so `stop` and `status` of a running
Bilterm's queue are colon, RPC and MCP only (`bilterm ctl speech.stop`).

## 6. How it works

```text
text -> normalise (numbers, symbols, pauses)
     -> letter-to-sound rules (NRL-style, ~350 rules + a small exception list)
     -> phonemes (40, ARPAbet) with stress, durations, pauses
     -> formant tracks (F1..F3, voicing, frication) + intonation contour
     -> source-filter synthesis at 16 kHz (buzz source + 5 resonators + noise band)
     -> voice colouring (ring modulator, echo, sample-and-hold, bit-crush) -> 16-bit PCM
```

The synthesizer uses only add, subtract, multiply and divide (its own sine and exponential), so the
output is bit-for-bit identical on Linux, macOS and Windows. Golden hashes in the unit tests pin it.

## 7. Known limits

- Rule-based English only; unusual names and loan words can sound wrong (that is part of the charm).
- Voices are tuned by numbers and spectral measurements, not by ear: expect SAM-grade intelligibility, not modern quality.
- Output is mono 16 kHz.
