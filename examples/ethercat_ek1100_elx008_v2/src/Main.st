PROGRAM Main
VAR
    DO0 : BOOL;
    DO1 : BOOL;
    DO2 : BOOL;
    DO3 : BOOL;
    DO4 : BOOL;
    DO5 : BOOL;
    DO6 : BOOL;
    DO7 : BOOL;

    step_timer : TON;  (* Temporizador para cambio de paso *)
    step_time : TIME := T#200MS;  (* Velocidad del efecto (200ms por paso) *)
    position : INT := 0;  (* Posicion actual (0-15) *)
    direction : INT := 1;  (* Direccion: 1=adelante, -1=atras *)
END_VAR

(* Temporizador para el siguiente paso *)
step_timer(IN := NOT step_timer.Q, PT := step_time);

(* Cuando el temporizador termina, avanzar al siguiente paso *)
IF step_timer.Q THEN
    position := position + direction;

    (* Si llegamos al final, invertir direccion *)
    IF position >= 15 THEN
        direction := -1;
    END_IF;

    (* Si llegamos al inicio, invertir direccion *)
    IF position <= 0 THEN
        direction := 1;
    END_IF;
END_IF;

(* Actualizar las salidas segun la posicion *)
IF position <= 7 THEN
    (* Fase de activacion: encender desde 0 hasta position *)
    DO0 := (position >= 0);
    DO1 := (position >= 1);
    DO2 := (position >= 2);
    DO3 := (position >= 3);
    DO4 := (position >= 4);
    DO5 := (position >= 5);
    DO6 := (position >= 6);
    DO7 := (position >= 7);
ELSE
    (* Fase de desactivacion: apagar desde 7 hasta 0 *)
    DO0 := (position < 15);
    DO1 := (position < 14);
    DO2 := (position < 13);
    DO3 := (position < 12);
    DO4 := (position < 11);
    DO5 := (position < 10);
    DO6 := (position < 9);
    DO7 := (position < 8);
END_IF;

END_PROGRAM
