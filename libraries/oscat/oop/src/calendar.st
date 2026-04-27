FUNCTION_BLOCK CalendarClock IMPLEMENTS ICalendarClock
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    CalendarData : CALENDAR;
    Holidays : ARRAY[0..29] OF HOLIDAY_DATA;
    CalendarFb : CALENDAR_CALC;
    SunPositionFb : SUN_POS;
    SunTimeFb : SUN_TIME;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
OSCAT_BASIC_Constants();
CalendarData.OFFSET := DefaultCalendarOffsetMinutes;
CalendarData.LANGUAGE := LANGUAGE.DEFAULT;
ClearError();
END_METHOD

METHOD PUBLIC Reset
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC Configure
VAR_INPUT
    LocationLatitude : REAL;
    LocationLongitude : REAL;
    OffsetMinutes : INT;
    DstEnabled : BOOL;
    LanguageIndex : INT;
END_VAR
IF NOT ReadyValue THEN
    Initialize();
END_IF;
CalendarData.LATITUDE := LocationLatitude;
CalendarData.LONGITUDE := LocationLongitude;
CalendarData.OFFSET := OffsetMinutes;
CalendarData.DST_EN := DstEnabled;
CalendarData.LANGUAGE := LanguageIndex;
ClearError();
END_METHOD

METHOD PUBLIC Update
VAR_INPUT
    Utc : DT;
END_VAR
IF NOT ReadyValue THEN
    Initialize();
END_IF;
CalendarData.UTC := Utc;
CalendarFb(SPE := TRUE, H := REAL#-0.8333333, XCAL := CalendarData, HOLIDAYS := Holidays);
END_METHOD

METHOD PUBLIC CalculateSunPosition : SunPosition
VAR_INPUT
    Utc : DT;
    Latitude : REAL;
    Longitude : REAL;
END_VAR
VAR
    Result : SunPosition;
END_VAR
OSCAT_BASIC_Constants();
SunPositionFb(LATITUDE := Latitude, LONGITUDE := Longitude, UTC := Utc);
Result.Azimuth := SunPositionFb.B;
Result.Elevation := SunPositionFb.H;
Result.RefractedElevation := SunPositionFb.HR;
CalculateSunPosition := Result;
END_METHOD

METHOD PUBLIC CalculateSunTime : SunTimes
VAR_INPUT
    UtcDate : DATE;
    Latitude : REAL;
    Longitude : REAL;
    Horizon : REAL;
END_VAR
VAR
    Result : SunTimes;
END_VAR
OSCAT_BASIC_Constants();
SunTimeFb(LATITUDE := Latitude, LONGITUDE := Longitude, UTC := UtcDate, H := Horizon);
Result.SolarNoon := SunTimeFb.MIDDAY;
Result.Sunrise := SunTimeFb.SUN_RISE;
Result.Sunset := SunTimeFb.SUN_SET;
Result.Declination := SunTimeFb.SUN_DECLINATION;
CalculateSunTime := Result;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Utc : DT
GET
    Utc := CalendarData.UTC;
END_GET
END_PROPERTY

PUBLIC PROPERTY LocalDateTime : DT
GET
    LocalDateTime := CalendarData.LOCAL_DT;
END_GET
END_PROPERTY

PUBLIC PROPERTY LocalDate : DATE
GET
    LocalDate := CalendarData.LOCAL_DATE;
END_GET
END_PROPERTY

PUBLIC PROPERTY LocalTimeOfDay : TOD
GET
    LocalTimeOfDay := CalendarData.LOCAL_TOD;
END_GET
END_PROPERTY

PUBLIC PROPERTY Year : INT
GET
    Year := CalendarData.YEAR;
END_GET
END_PROPERTY

PUBLIC PROPERTY Month : INT
GET
    Month := CalendarData.MONTH;
END_GET
END_PROPERTY

PUBLIC PROPERTY Day : INT
GET
    Day := CalendarData.DAY;
END_GET
END_PROPERTY

PUBLIC PROPERTY Weekday : INT
GET
    Weekday := CalendarData.WEEKDAY;
END_GET
END_PROPERTY

PUBLIC PROPERTY OffsetMinutes : INT
GET
    OffsetMinutes := CalendarData.OFFSET;
END_GET
END_PROPERTY

PUBLIC PROPERTY DstEnabled : BOOL
GET
    DstEnabled := CalendarData.DST_EN;
END_GET
END_PROPERTY

PUBLIC PROPERTY DstOn : BOOL
GET
    DstOn := CalendarData.DST_ON;
END_GET
END_PROPERTY

PUBLIC PROPERTY LanguageIndex : INT
GET
    LanguageIndex := CalendarData.LANGUAGE;
END_GET
END_PROPERTY

PUBLIC PROPERTY Longitude : REAL
GET
    Longitude := CalendarData.LONGITUDE;
END_GET
END_PROPERTY

PUBLIC PROPERTY Latitude : REAL
GET
    Latitude := CalendarData.LATITUDE;
END_GET
END_PROPERTY

PUBLIC PROPERTY Sunrise : TOD
GET
    Sunrise := CalendarData.SUN_RISE;
END_GET
END_PROPERTY

PUBLIC PROPERTY Sunset : TOD
GET
    Sunset := CalendarData.SUN_SET;
END_GET
END_PROPERTY

PUBLIC PROPERTY SolarNoon : TOD
GET
    SolarNoon := CalendarData.SUN_MIDDAY;
END_GET
END_PROPERTY

PUBLIC PROPERTY SolarElevation : REAL
GET
    (* OSCAT classic keeps the historical SUN_HEIGTH spelling. *)
    SolarElevation := CalendarData.SUN_HEIGTH;
END_GET
END_PROPERTY

PUBLIC PROPERTY SolarHorizontalProjection : REAL
GET
    SolarHorizontalProjection := CalendarData.SUN_HOR;
END_GET
END_PROPERTY

PUBLIC PROPERTY SolarVerticalProjection : REAL
GET
    SolarVerticalProjection := CalendarData.SUN_VER;
END_GET
END_PROPERTY

PUBLIC PROPERTY Night : BOOL
GET
    Night := CalendarData.NIGHT;
END_GET
END_PROPERTY

PUBLIC PROPERTY Holiday : BOOL
GET
    Holiday := CalendarData.HOLIDAY;
END_GET
END_PROPERTY

PUBLIC PROPERTY HolidayName : STRING[30]
GET
    HolidayName := CalendarData.HOLY_NAME;
END_GET
END_PROPERTY

PUBLIC PROPERTY WorkWeek : INT
GET
    WorkWeek := CalendarData.WORK_WEEK;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK HolidayCalendar IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    HolidayValue : BOOL;
    NameValue : STRING[30];
    LanguageIndexValue : INT := INT#1;
    FridayAsHolidayValue : BOOL;
    SaturdayAsHolidayValue : BOOL;
    SundayAsHolidayValue : BOOL;
    Holidays : ARRAY[0..29] OF HOLIDAY_DATA;
    HolidayFb : HOLIDAY;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
OSCAT_BASIC_Constants();
ClearError();
END_METHOD

METHOD PUBLIC Reset
HolidayValue := FALSE;
NameValue := '';
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC Configure
VAR_INPUT
    LanguageIndex : INT;
    FridayAsHoliday : BOOL;
    SaturdayAsHoliday : BOOL;
    SundayAsHoliday : BOOL;
END_VAR
LanguageIndexValue := LanguageIndex;
FridayAsHolidayValue := FridayAsHoliday;
SaturdayAsHolidayValue := SaturdayAsHoliday;
SundayAsHolidayValue := SundayAsHoliday;
ClearError();
END_METHOD

METHOD PUBLIC SetHoliday
VAR_INPUT
    Index : INT;
    Month : SINT;
    Day : SINT;
    Use : SINT;
    Name : STRING[30];
END_VAR
IF Index < INT#0 OR Index > INT#29 THEN
    ErrorValue := TRUE;
    ErrorIdValue := ComponentErrorInvalidConfiguration;
    StatusValue := BYTE#1;
ELSE
    Holidays[Index].MONTH := Month;
    Holidays[Index].DAY := Day;
    Holidays[Index].USE := Use;
    Holidays[Index].NAME := Name;
    ClearError();
END_IF;
END_METHOD

METHOD PUBLIC Update : BOOL
VAR_INPUT
    DateIn : DATE;
END_VAR
HolidayFb(
    DATE_IN := DateIn,
    LANGU := LanguageIndexValue,
    FRIDAY := FridayAsHolidayValue,
    SATURDAY := SaturdayAsHolidayValue,
    SUNDAY := SundayAsHolidayValue,
    HOLIDAYS := Holidays
);
HolidayValue := HolidayFb.Y;
NameValue := HolidayFb.NAME;
Update := HolidayValue;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Holiday : BOOL
GET
    Holiday := HolidayValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Name : STRING[30]
GET
    Name := NameValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK

FUNCTION_BLOCK RtcClock IMPLEMENTS IComponent
VAR
    ReadyValue : BOOL;
    ErrorValue : BOOL;
    ErrorIdValue : WORD;
    StatusValue : BYTE;
    UtcDateTimeValue : DT;
    LocalDateTimeValue : DT;
    DstOnValue : BOOL;
    MillisecondsValue : INT;
    DstEnabledValue : BOOL;
    OffsetMinutesValue : INT;
    RtcFb : RTC_2;
END_VAR

METHOD PUBLIC Initialize
ReadyValue := TRUE;
ClearError();
END_METHOD

METHOD PUBLIC Reset
UtcDateTimeValue := DT#1970-01-01-00:00:00;
LocalDateTimeValue := DT#1970-01-01-00:00:00;
MillisecondsValue := INT#0;
ClearError();
END_METHOD

METHOD PUBLIC ClearError
ErrorValue := FALSE;
ErrorIdValue := ComponentErrorNone;
StatusValue := BYTE#0;
END_METHOD

METHOD PUBLIC Snapshot : ComponentStatus
VAR
    Result : ComponentStatus;
END_VAR
Result.Ready := ReadyValue;
Result.Error := ErrorValue;
Result.ErrorId := ErrorIdValue;
Result.Status := StatusValue;
Snapshot := Result;
END_METHOD

METHOD PUBLIC SetClock
VAR_INPUT
    InitialDateTime : DT;
    Milliseconds : INT;
    DstEnabled : BOOL;
    OffsetMinutes : INT;
END_VAR
DstEnabledValue := DstEnabled;
OffsetMinutesValue := OffsetMinutes;
RtcFb(SET_RTC := TRUE, SDT := InitialDateTime, SMS := Milliseconds, DEN := DstEnabledValue, OFS := OffsetMinutesValue);
UtcDateTimeValue := RtcFb.UDT;
LocalDateTimeValue := RtcFb.LOCAL_DT;
DstOnValue := RtcFb.DSO;
MillisecondsValue := RtcFb.XMS;
ClearError();
END_METHOD

METHOD PUBLIC Update
RtcFb(SET_RTC := FALSE, SDT := UtcDateTimeValue, SMS := MillisecondsValue, DEN := DstEnabledValue, OFS := OffsetMinutesValue);
UtcDateTimeValue := RtcFb.UDT;
LocalDateTimeValue := RtcFb.LOCAL_DT;
DstOnValue := RtcFb.DSO;
MillisecondsValue := RtcFb.XMS;
END_METHOD

PUBLIC PROPERTY Ready : BOOL
GET
    Ready := ReadyValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Error : BOOL
GET
    Error := ErrorValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY ErrorId : WORD
GET
    ErrorId := ErrorIdValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Status : BYTE
GET
    Status := StatusValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY UtcDateTime : DT
GET
    UtcDateTime := UtcDateTimeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY LocalDateTime : DT
GET
    LocalDateTime := LocalDateTimeValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY DstOn : BOOL
GET
    DstOn := DstOnValue;
END_GET
END_PROPERTY

PUBLIC PROPERTY Milliseconds : INT
GET
    Milliseconds := MillisecondsValue;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
