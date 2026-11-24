; Inno Setup script for the Windows build of MultiDeck.
;
; Build the executable first, then compile this script:
;
;   cargo build --release -p multideck-desktop
;   ISCC.exe packaging\windows\multideck.iss
;
; scripts\build-installer.ps1 runs both steps and leaves the installer in dist\.

#define AppName "MultiDeck"
#define AppVersion "0.1.0"
#define AppPublisher "MultiDeck contributors"
#define AppExe "multideck.exe"

[Setup]
AppId={{6A1D3F04-8B7E-4C55-9E31-4E6C0B0D9A17}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\MultiDeck
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
OutputDir=..\..\dist
OutputBaseFilename=MultiDeck-{#AppVersion}-setup
SetupIconFile=..\..\assets\icons\icon.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
LicenseFile=..\..\LICENSE

[Files]
Source: "..\..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
