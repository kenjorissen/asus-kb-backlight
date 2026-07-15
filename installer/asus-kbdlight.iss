#define MyAppName "ASUS Keyboard Backlight"
#define MyAppVersion "0.1.6"
#define MyAppPublisher "ASUS Keyboard Backlight Utility"
#define MyAppExeName "asus-kbdlight.exe"
#define MyTrayExeName "asus-kbdlight-tray.exe"

[Setup]
AppId={{5E803189-D2E7-41F9-B213-3A957D270FA7}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\AsusKbdLight
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
OutputDir=..\dist
OutputBaseFilename=AsusKbdLightSetup-{#MyAppVersion}-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#MyAppExeName}
; The tray owns a hidden message window that Restart Manager cannot close.
; PrepareToInstall below explicitly stops our two processes before replacement.
CloseApplications=no
RestartApplications=no
SetupLogging=yes
SetupIconFile=..\assets\asus-kbdlight.ico

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#MyTrayExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKLM; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "AsusKbdLightTray"; ValueData: """{app}\{#MyTrayExeName}"""; Flags: uninsdeletevalue

[Icons]
Name: "{autoprograms}\ASUS Keyboard Backlight Status"; Filename: "{app}\{#MyTrayExeName}"; Parameters: "status"; WorkingDir: "{app}"
Name: "{autoprograms}\ASUS Keyboard Backlight Tray"; Filename: "{app}\{#MyTrayExeName}"; WorkingDir: "{app}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Parameters: "service install"; StatusMsg: "Installing and starting the keyboard backlight service..."; Flags: runhidden waituntilterminated
Filename: "{app}\{#MyTrayExeName}"; Description: "Start the keyboard backlight tray icon"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#MyTrayExeName} /F"; Flags: runhidden waituntilterminated; RunOnceId: "StopTray"
Filename: "{app}\{#MyAppExeName}"; Parameters: "service uninstall"; Flags: runhidden waituntilterminated skipifdoesntexist; RunOnceId: "RemoveService"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';

  { The tray is stateless between menu clicks, so forced termination is safe. }
  Exec(
    ExpandConstant('{sys}\taskkill.exe'),
    '/F /IM {#MyTrayExeName}',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );

  if FileExists(ExpandConstant('{app}\{#MyAppExeName}')) then
  begin
    Exec(
      ExpandConstant('{app}\{#MyAppExeName}'),
      'service stop',
      '',
      SW_HIDE,
      ewWaitUntilTerminated,
      ResultCode
    );
    Sleep(1000);
  end;

  { Clean up an older service build whose stop command returned too early. }
  Exec(
    ExpandConstant('{sys}\taskkill.exe'),
    '/F /IM {#MyAppExeName}',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
end;
