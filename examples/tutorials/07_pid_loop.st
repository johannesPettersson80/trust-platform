PROGRAM PidLoop
VAR
    setpoint : REAL := 50.0;
    process_value : REAL := 40.0;
    kp : REAL := 1.2;
    ki : REAL := 0.1;
    kd : REAL := 0.05;
    sample_time : REAL := 0.1;
    integral_term : REAL := 0.0;
    previous_error : REAL := 0.0;
    error : REAL := 0.0;
    derivative : REAL := 0.0;
    control_output : REAL := 0.0;
END_VAR

error := setpoint - process_value;
integral_term := integral_term + (error * ki * sample_time);
derivative := (error - previous_error) / sample_time;
control_output := (kp * error) + integral_term + (kd * derivative);
previous_error := error;
process_value := process_value + (control_output * 0.02);
END_PROGRAM
