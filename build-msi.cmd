@echo off
setlocal EnableExtensions EnableDelayedExpansion

pushd "%~dp0" || exit /b 1

set "INSTALL_AFTER_BUILD=0"
if /i "%~1"=="--install" set "INSTALL_AFTER_BUILD=1"
if /i "%~1"=="/install" set "INSTALL_AFTER_BUILD=1"
if not "%~1"=="" if "%INSTALL_AFTER_BUILD%"=="0" (
    echo Usage: build-msi.cmd [--install]
    goto :failure
)

where cargo.exe >nul 2>&1
if errorlevel 1 (
    echo ERROR: Cargo was not found. Install the Rust toolchain from https://rustup.rs/.
    goto :failure
)

where wix.exe >nul 2>&1
if errorlevel 1 (
    echo ERROR: WiX Toolset 7 was not found.
    echo Install it with: dotnet tool install --global wix --version 7.0.0
    goto :failure
)

set "APP_VERSION="
for /f "tokens=3" %%V in ('findstr /b /c:"version = " Cargo.toml') do if not defined APP_VERSION set "APP_VERSION=%%~V"
if not defined APP_VERSION (
    echo ERROR: Could not read the package version from Cargo.toml.
    goto :failure
)

set "MSI_NAME=Goatpad-%APP_VERSION%-x64.msi"
set "MSI_PATH=dist\%MSI_NAME%"

echo [1/3] Building Goatpad %APP_VERSION% in release mode...
cargo build --release
if errorlevel 1 goto :failure

echo [2/3] Preparing the WiX UI extension...
wix extension add WixToolset.UI.wixext/7.0.0
if errorlevel 1 goto :failure

if not exist dist mkdir dist

echo [3/3] Building %MSI_NAME%...
wix build wix\main.wxs -arch x64 -ext WixToolset.UI.wixext -d ProductVersion=%APP_VERSION% -out "%MSI_PATH%" -pdbtype none
if errorlevel 1 goto :failure

echo.
echo MSI created successfully:
echo   %CD%\%MSI_PATH%

if "%INSTALL_AFTER_BUILD%"=="1" (
    echo.
    echo Starting Windows Installer. Close Goatpad before continuing.
    start /wait "" msiexec.exe /i "%CD%\%MSI_PATH%"
    set "INSTALL_RESULT=!ERRORLEVEL!"
    if "!INSTALL_RESULT!"=="0" goto :installed
    if "!INSTALL_RESULT!"=="3010" goto :restart_required
    echo ERROR: Windows Installer exited with code !INSTALL_RESULT!.
    goto :failure
)

goto :success

:installed
echo Goatpad %APP_VERSION% was installed successfully.
goto :success

:restart_required
echo Goatpad %APP_VERSION% was installed successfully. Windows must be restarted.
goto :success

:failure
echo.
echo MSI build did not complete.
popd
exit /b 1

:success
popd
exit /b 0
