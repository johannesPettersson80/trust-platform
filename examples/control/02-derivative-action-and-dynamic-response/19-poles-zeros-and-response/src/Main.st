FUNCTION Lesson19StablePole : LREAL
VAR_INPUT
    TimeConstant : LREAL;
END_VAR
Lesson19StablePole := -LREAL#1.0 / TimeConstant;
END_FUNCTION

FUNCTION Lesson19OscillatingPoleRealPart : LREAL
VAR_INPUT
    NaturalFrequency : LREAL;
    DampingRatio : LREAL;
END_VAR
Lesson19OscillatingPoleRealPart := -DampingRatio * NaturalFrequency;
END_FUNCTION

FUNCTION Lesson19OscillatingPoleImaginaryPart : LREAL
VAR_INPUT
    NaturalFrequency : LREAL;
    DampingRatio : LREAL;
END_VAR
Lesson19OscillatingPoleImaginaryPart := NaturalFrequency
    * SQRT(LREAL#1.0 - DampingRatio * DampingRatio);
END_FUNCTION

FUNCTION Lesson19InverseResponse : LREAL
VAR_INPUT
    ElapsedTime : LREAL;
END_VAR
Lesson19InverseResponse := LREAL#2.0
    * (LREAL#1.0 - EXP(-ElapsedTime / LREAL#20.0))
    - (LREAL#1.0 - EXP(-ElapsedTime / LREAL#2.0));
END_FUNCTION

PROGRAM Main
VAR
    ExperimentEnabled : BOOL := FALSE;
    CommonInput : LREAL := 0.0;
    StableResponse : LREAL := 0.0;
    IntegratingResponse : LREAL := 0.0;
    OscillatingResponse : LREAL := 0.0;
    InversePositivePath : LREAL := 0.0;
    InverseNegativePath : LREAL := 0.0;
    InverseResponse : LREAL := 0.0;
    StablePole : LREAL := -0.2;
    OscillatingPoleRealPart : LREAL := -0.3;
    OscillatingPoleImaginaryPart : LREAL := 1.47;
END_VAR

IF ExperimentEnabled THEN
    CommonInput := LREAL#1.0;
ELSE
    CommonInput := LREAL#0.0;
END_IF;

StablePole := Lesson19StablePole(TimeConstant := LREAL#25.0);
OscillatingPoleRealPart := Lesson19OscillatingPoleRealPart(
    NaturalFrequency := LREAL#1.5,
    DampingRatio := LREAL#0.2
);
OscillatingPoleImaginaryPart := Lesson19OscillatingPoleImaginaryPart(
    NaturalFrequency := LREAL#1.5,
    DampingRatio := LREAL#0.2
);
(* The inverse response is the measured sum of a slow positive path and a
   fast negative path driven by the same input. *)
InverseResponse := InversePositivePath + InverseNegativePath;
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
VAR_CONFIG
    P1.CommonInput AT %QL0 : LREAL;
    P1.StableResponse AT %IL0 : LREAL;
    P1.IntegratingResponse AT %IL8 : LREAL;
    P1.OscillatingResponse AT %IL16 : LREAL;
    P1.InversePositivePath AT %IL24 : LREAL;
    P1.InverseNegativePath AT %IL32 : LREAL;
    P1.InverseResponse AT %ML0 : LREAL;
    P1.StablePole AT %ML8 : LREAL;
    P1.OscillatingPoleRealPart AT %ML16 : LREAL;
    P1.OscillatingPoleImaginaryPart AT %ML24 : LREAL;
    P1.ExperimentEnabled AT %MX80.0 : BOOL;
END_VAR
END_CONFIGURATION
