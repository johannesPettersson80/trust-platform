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
    Language : INT;
END_VAR
IF NOT ReadyValue THEN
    Initialize();
END_IF;
CalendarData.LATITUDE := LocationLatitude;
CalendarData.LONGITUDE := LocationLongitude;
CalendarData.OFFSET := OffsetMinutes;
CalendarData.DST_EN := DstEnabled;
CalendarData.LANGUAGE := Language;
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
Result.Height := SunPositionFb.H;
Result.RefractedHeight := SunPositionFb.HR;
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
Result.Midday := SunTimeFb.MIDDAY;
Result.Rise := SunTimeFb.SUN_RISE;
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

PUBLIC PROPERTY SunRise : TOD
GET
    SunRise := CalendarData.SUN_RISE;
END_GET
END_PROPERTY

PUBLIC PROPERTY SunSet : TOD
GET
    SunSet := CalendarData.SUN_SET;
END_GET
END_PROPERTY

PUBLIC PROPERTY SunMidday : TOD
GET
    SunMidday := CalendarData.SUN_MIDDAY;
END_GET
END_PROPERTY

PUBLIC PROPERTY SunHeight : REAL
GET
    SunHeight := CalendarData.SUN_HEIGTH;
END_GET
END_PROPERTY

PUBLIC PROPERTY SunHorizontal : REAL
GET
    SunHorizontal := CalendarData.SUN_HOR;
END_GET
END_PROPERTY

PUBLIC PROPERTY SunVertical : REAL
GET
    SunVertical := CalendarData.SUN_VER;
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
