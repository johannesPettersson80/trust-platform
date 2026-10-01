# Control course lesson template and review checklist

Every lesson is written to the standard of the reference lessons **1. Control-loop elements**, **2. ON/OFF control**,
**3. Hysteresis and dwell**, **4. Cold-room commissioning** and **5. Proportional control**. This template states what
those lessons do, so every other lesson can be written and checked the same way. It applies the
[Control Course Teaching Specification](control-course-teaching-specification.md) §4 and §5; where they differ, that
specification governs.

## 1. The chain rule

- A lesson starts with the problem the previous lesson ended with, solves exactly that problem, and ends with the
  problem the next lesson solves.
- **The problem** names only the previous lesson. It does not lean on older lessons. An assessment (4, 30, 41, 68, 89)
  may name the lessons it combines, as Lesson 4 names Lessons 1–3.
- A lesson keeps the previous lesson's plant, operating point and numbers. Any change is stated with its reason, as
  Lesson 5 states the move from the cold room to the heater.
- A control term is used only from the lesson that defines it onwards. A later lesson may use it without redefining it.

## 2. How each lesson must look

The title, a direct opening and the sections, in this order. Length follows the explanation: a good explanation is
better than a short one (user, 1 October 2026). Keep the other sections short and to the point.

### Title and opening
- `# N. <Concept>`: the concept's short technical name ("Proportional control", "Hysteresis and dwell").
- One or two sentences: what the student will do, and the limitation the experiment will reveal.
  *Lesson 5: "In this lesson, you will make the heater-power command increase with the difference between the
  temperature setpoint and measured water temperature. You will then observe why this correction alone cannot hold the
  temperature setpoint while heat is continually leaving the system."*

### 1. The problem
- Starts from the previous lesson's result, with its numbers. *Lesson 3: "Lesson 2 used one temperature for both
  switching decisions. … The command changed state 760 times in 120 s."*
- Then the new physical requirement or limitation, in plain words.
- Ends with what this lesson will do: "We will …". Two short paragraphs.

### 2. What we need
- The missing capability in plain words, often as "We need to: 1. … 2. … 3. …". No parameters or settings.

### 3. The new idea
The explanation always has these four steps, in this order (the user's approved model is lesson 74, State-space models):

1. **Definition.** The bold term and its plain-words definition from the lesson map, in one or two sentences.
2. **In words.** What the idea means, explained without formulas: its parts, what each part does, a comparison the
   audience knows (for example PLC scan variables), and what it means for this plant.
3. **The formula.** The general formula, then every symbol in a list with its meaning and unit.
4. **Example.** The lesson's own plant with real numbers, worked step by step, with a number after every formula and a
   sentence saying what each result means.

Then the limitation, calculated, and any teaching choices, labelled. The rules below apply inside these steps.

- **First sentence: the bold term and a plain definition.** *"**Proportional control** makes the correcting command
  proportional to the current difference between the desired and measured value."*
- **Second sentence: what it means for this plant.** *"For this heater, a larger temperature shortfall produces more
  heating."*
- **The physics before any formula:** why the plant behaves as it does (stored heat, energy in and out).
- **Rules as small tables** where they help (Lesson 2's switching table).
- **Symbols introduced as a list with units** ("Let: *e* be the temperature error, in degrees Celsius; …").
- **Every formula followed at once by a worked number** and what the result means. *"With SP = 2.0 °C and PV = 4.0 °C,
  e = 2.0 − 4.0 = −2.0 °C. The negative sign says that the room is warmer than the desired value."*
- **Each new term in bold at first use with a plain definition.** *"Heat entering through the open door is a
  **disturbance**: an outside influence that changes the temperature we want to control."*
- **Units explained in words.** *"Two percentage points means, for example, increasing a command from 20% to 22%."*
- **Dynamic behaviour as a numbered walk-through** (*"Follow the repeated decision: 1. … 2. … 3. …"*).
- **Teaching choices labelled as such.** *"This is a teaching choice … not a tuned value for physical equipment."*
- **The method's limitation explained and calculated**, as Lesson 5 calculates the steady-state error from the heat
  balance.
- **Only algebra and basic calculus.** Anything more is introduced with enough reasoning to follow the worked
  calculation.
- **Math you need.** Where a mathematical tool appears for the first time in the course (a sum, an exponential, a
  logarithm, complex numbers, a matrix, a variance), a short box before its first use explains it with one small
  example. The lesson map lists where each box goes.

### 4. Prediction
- "Before running, answer these questions:" 3–6 numbered questions, each answerable from the new idea.
- Numeric predictions as a small worked table where possible (Lesson 5's error and command at 20, 30 and 35 °C).
- Where exact values are not yet possible, say so, as Lesson 1 does.

### 5. Implementation
- Which file to open and what to find in it.
- One concrete ST task tied to the concept (**ST practice** or **Exercise**), with a short code block and the values to
  check before the experiment. A new ST construct is explained in one sentence; known ST is not explained.
- How the program runs the method (for example `Control.PID` as P with its settings), the plant's delay and the safety
  abort.

### 6. Experiment in Control Studio
A lesson that introduces a plant first shows it in the hardware canvas: the plant, its inputs and outputs, and how it
is connected to the program's signals.

Numbered steps with the exact Control Studio names, checked in a real run:
- start the simulator and confirm the initial state;
- the page, the method and the signals to show, with what each signal means;
- the predictions to answer before starting;
- **Review project test** and the declared duration, and what that duration covers;
- **Confirm and start** as time zero, and the events with their times;
- what to watch or hover;
- the end state to confirm, when and how to abort, and how to repeat fairly.

### 7. Evidence and decision
- **One annotated figure:** the recorded response from the real run, with numbered marks; each number answers the
  prediction with the same number (for example where the temperature crosses the setpoint, the peak, where it
  settles). The lesson's native test runs the same project and keeps its trace; the lesson's
  `figures/generate_figures.py` draws the figure from that trace. The Control Studio run must record the same values.
- A table of the key values from the run.
- Each prediction answered, in order.
- Why it happened, and its cost as well as its benefit (*"The wider temperature movement is the cost of waiting for
  that travel."*).
- The decision: does it solve the problem, and what remains? Simulation results are stated as simulation results.

### 8. Conclusion
- Two short paragraphs: the principle learned, then the result for this plant.

### Check yourself (after the conclusion, before the reason)
- Three short questions on this lesson's idea, each followed by its answer, so the student can test understanding
  without help. At least one asks for a small calculation with the lesson's numbers; one asks why something happened.
- Answers are short and complete: the result and the one-sentence reason.

### 9. Reason for the next lesson
- What remains unresolved now, in physical terms.
- One sentence linking the next lesson and how it solves that problem. *"[PI temperature control](…) adds a stored
  correction that changes while error remains, forming a PI controller that can supply holding power at zero error."*

### Language
- Short connected paragraphs; plain words; standard terms explained at first use.
- No slogans, administrative wording, expert shorthand, unexplained formulas or repeated disclaimers.
- Every number checked against the run; units, signs and limiting cases checked.

## 3. What is checked for every lesson

### Checked automatically, for all lessons at once
1. The title is `# N. <Concept>` and matches the catalog and `lesson.json`.
2. The nine section headings are present, in order.
3. **Chain:** the previous lesson's **Reason for the next lesson** links to this lesson; **The problem** names the
   previous lesson and no other (assessments excepted).
4. **Terms:** every bold defined term has exactly one defining lesson, and no lesson uses a term before that lesson.
5. **The new idea** starts with a bold term followed by its definition.
6. **The new idea** has the four steps: definition, in words, the formula with every symbol explained, the worked
   example. **Check yourself** has three questions, each with an answer.
7. **Math you need** boxes are present in the lessons the map lists.
8. `lesson.json` is synced from the README and passes the lesson validator; links resolve; the ST tests and the
   lesson's native test pass, including the test that writes the annotated figure.

### Checked by reading, one lesson at a time
1. **The problem** continues from the previous lesson's result, with its numbers, and ends with "We will …".
2. **What we need** states a capability, not settings.
3. **The new idea** follows section 2 above: definition, plant meaning, physics before formulas, symbols with units,
   worked numbers, terms defined, teaching choices labelled, limitation calculated, basic mathematics only.
4. Every prediction is answerable from the new idea and is answered in **Evidence and decision**.
5. The ST task is concrete and tied to the concept.
6. The Control Studio steps are complete and match a real run; every number in the lesson comes from that run.
7. **Conclusion** and **Reason for the next lesson** match what was shown, and the reason matches the next lesson's
   problem.
8. It reads like Lessons 1–5: plain, physical, concrete, nothing a beginner must already know.

A lesson is finished when it passes every automatic check and every reading check, and the user has approved it.

## 4. Decisions

- **1 October 2026:** each lesson adds **Check yourself**; **Math you need** boxes where a mathematical tool first
  appears; one annotated figure per lesson; a lesson that introduces a plant shows it in the hardware canvas.
- **Pilot:** lessons 1–10 were written first. On 1 October the user liked them and asked for all remaining lessons to
  be written the same way now; two or three PLC engineers try the complete course afterwards.
