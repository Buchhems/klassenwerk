; Klassenwerk – Windows-Installer (Inno Setup 6, https://jrsoftware.org/isinfo.php)
; Bauen:  powershell -File packaging\windows\build-windows.ps1
; oder:   iscc /DAppVersion=1.0.0 packaging\windows\klassenwerk.iss
;
; Installiert ohne Administratorrechte nach %LOCALAPPDATA%\Programs\Klassenwerk
; (wahlweise für alle Benutzer). Die Daten in %APPDATA%\Klassenwerk bleiben bei
; Update und Deinstallation erhalten.

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\app\target\release\Klassenwerk.exe"
#endif

[Setup]
; feste ID: Updates ersetzen die vorhandene Installation
AppId={{47A83248-7C94-40BE-B1BC-F74C8EAF4B5B}
AppName=Klassenwerk
AppVersion={#AppVersion}
AppVerName=Klassenwerk {#AppVersion}
AppPublisher=Klassenwerk
AppComments=Aufgabendatenbank, Klausurplaner und Notenbuch für Lehrkräfte
DefaultDirName={autopf}\Klassenwerk
DefaultGroupName=Klassenwerk
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
SetupIconFile=..\..\app\assets\windows\klassenwerk.ico
UninstallDisplayIcon={app}\Klassenwerk.exe
UninstallDisplayName=Klassenwerk
OutputDir=..\..\dist
OutputBaseFilename=Klassenwerk-{#AppVersion}-windows-setup
VersionInfoVersion={#AppVersion}
VersionInfoProductName=Klassenwerk
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "de"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "Klassenwerk.exe"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_LICENSES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Klassenwerk"; Filename: "{app}\Klassenwerk.exe"; Comment: "Aufgaben, Klausuren & Noten"
Name: "{autodesktop}\Klassenwerk"; Filename: "{app}\Klassenwerk.exe"; Tasks: desktopicon

[Run]
; also after a silent update started from Klassenwerk (/SILENT): start the program again
Filename: "{app}\Klassenwerk.exe"; Description: "{cm:LaunchProgram,Klassenwerk}"; Flags: nowait postinstall

[Messages]
de.FinishedLabel=Klassenwerk wurde installiert.%n%nIhre Aufgaben und Klausuren werden in %APPDATA%\Klassenwerk gespeichert und bleiben auch bei einer Deinstallation erhalten.
