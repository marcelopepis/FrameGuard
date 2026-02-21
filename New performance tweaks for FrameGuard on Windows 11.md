# New performance tweaks for FrameGuard on Windows 11

FrameGuard can add **28 legitimate, evidence-based optimizations** beyond its current feature set. The highest-impact additions are disabling Game DVR background recording, enabling the Ultimate Performance power plan, setting the `GlobalTimerResolutionRequests` registry key, enabling MSI mode for the GPU, and disabling telemetry services and scheduled tasks that cause periodic CPU spikes. Several commonly recommended tweaks — including IRQ priority manipulation, Nagle's algorithm changes, and disabling Spectre/Meltdown mitigations — are either obsolete, irrelevant to most games, or actively harmful on modern hardware.

The findings below draw from Chris Titus Tech's WinUtil source code, Tiny11 Builder's registry modifications, NVIDIA driver internals, Blur Busters latency research, and community benchmarking. Each tweak includes exact registry paths, PowerShell commands, an impact rating, and a snake oil assessment.

---

## GPU optimizations: driver-level and display pipeline

**1. Disable Game DVR / Background Recording** — Impact: **High**

Disabling the background video buffer frees GPU encoder resources and eliminates **1–3% CPU overhead** in CPU-bound titles. This is distinct from Game Mode (which FrameGuard already toggles).

```
HKCU:\System\GameConfigStore
  GameDVR_Enabled = 0 (DWORD)

HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR
  AllowGameDVR = 0 (DWORD)

HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\GameDVR
  AppCaptureEnabled = 0 (DWORD)
```

**2. Disable Xbox Game Bar Overlay** — Impact: **Low**

The overlay itself consumes negligible resources when not displayed (~300–900 KB RAM), but disabling it removes `GameBarPresenceWriter.exe` and prevents accidental activation during gameplay.

```
HKCU:\SOFTWARE\Microsoft\GameBar
  UseNexusForGameBarEnabled = 0 (DWORD)
  ShowStartupPanel = 0 (DWORD)
```

Optionally uninstall entirely via PowerShell:
```powershell
Get-AppxPackage Microsoft.XboxGamingOverlay | Remove-AppxPackage
Get-AppxPackage Microsoft.XboxGameOverlay | Remove-AppxPackage
```

**3. Enable MSI Mode for GPU** — Impact: **Medium**

Message Signaled Interrupts bypass the legacy PIC interrupt controller, reducing DPC latency. RTX 40-series cards ship with MSI enabled, but older GPUs (30-series and prior) often default to line-based interrupts. This tweak measurably reduces DPC latency on affected cards and resolves HDMI audio crackling.

```powershell
# Find NVIDIA GPU device path
$gpu = Get-PnpDevice | Where-Object { $_.FriendlyName -like "*NVIDIA*" -and $_.Class -eq "Display" }
$msiPath = "HKLM:\SYSTEM\CurrentControlSet\Enum\$($gpu.InstanceId)\Device Parameters\Interrupt Management\MessageSignaledInterruptProperties"
if (-not (Test-Path $msiPath)) { New-Item -Path $msiPath -Force }
New-ItemProperty -Path $msiPath -Name "MSISupported" -Value 1 -PropertyType DWord -Force
```

Also apply to the GPU's HD Audio controller for best results. Requires reboot.

**4. Disable Multiplane Overlay (MPO)** — Impact: **Medium**

MPO causes stuttering and flickering on certain GPU + monitor combinations, particularly with mixed-refresh-rate multi-monitor setups. WinUtil includes this tweak. The fix is a single registry value.

```
HKLM:\SOFTWARE\Microsoft\Windows\Dwm
  OverlayTestMode = 5 (DWORD)
```

**5. NVIDIA Telemetry Disable** — Impact: **Low**

Eliminates background CPU and network usage from NVIDIA telemetry processes. Safe to apply with no impact on driver functionality.

```powershell
# Registry telemetry opt-out
New-ItemProperty -Path "HKLM:\SOFTWARE\NVIDIA Corporation\Global\FTS" -Name "EnableRID44231" -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\NVIDIA Corporation\Global\FTS" -Name "EnableRID64640" -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\NVIDIA Corporation\Global\FTS" -Name "EnableRID66610" -Value 0 -PropertyType DWord -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\NVIDIA Corporation\NvControlPanel2\Client" -Name "OptInOrOutPreference" -Value 0 -PropertyType DWord -Force

# Disable NvTelemetryContainer service
Stop-Service -Name "NvTelemetryContainer" -Force -ErrorAction SilentlyContinue
Set-Service -Name "NvTelemetryContainer" -StartupType Disabled -ErrorAction SilentlyContinue
```

**6. Disable GPU Energy Tracking Driver** — Impact: **Low**

Removes the Windows GPU energy telemetry driver overhead. No user-facing impact.

```
HKLM:\SYSTEM\CurrentControlSet\Services\GpuEnergyDrv
  Start = 4 (DWORD)  # 4 = Disabled
```

**7. NVIDIA Driver Performance Registry Keys** — Impact: **Medium** (competitive gaming)

These driver-level tweaks are used by latency-focused communities. `DisableDynamicPstate` locks GPU clocks at maximum, bypassing the downclocking that even "Prefer Maximum Performance" in NVCP allows. The GraphicsDrivers keys reduce scheduler overhead.

```powershell
# Lock GPU at max frequency (increases idle power/heat)
$nvidiaPath = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000"
Set-ItemProperty -Path $nvidiaPath -Name "DisableDynamicPstate" -Value 1 -Type DWord

# Graphics driver performance settings
$gdPath = "HKLM:\SYSTEM\CurrentControlSet\Control\GraphicsDrivers"
New-ItemProperty -Path $gdPath -Name "RmGpsPsEnablePerCpuCoreDpc" -Value 1 -PropertyType DWord -Force
```

**Note on \0000 path**: This suffix corresponds to the first display adapter. Verify by checking `DriverDesc` in the key matches your NVIDIA GPU. If an iGPU is present, the dGPU may be `\0001`.

**NVIDIA ReBAR**: Cannot be enabled via registry. Requires BIOS settings (Above 4G Decoding, CSM disabled) + GPU VBIOS support + driver R465+. Not implementable programmatically.

**NVIDIA Control Panel profile settings** (power management, texture filtering, low latency): Stored as binary blobs via the NVAPI DRS system. Direct registry editing is fragile and undocumented. The practical approach is bundling NvidiaProfileInspector with a `.nip` preset file and importing via command line: `nvidiaProfileInspector.exe -silentImport "FrameGuard_Optimized.nip"`.

---

## CPU and power: plans, throttling, and timer resolution

**8. Ultimate Performance Power Plan** — Impact: **High**

Sets minimum processor state to 100%, disables hard disk sleep, and maximizes timer frequency. Hidden by default on non-workstation editions.

```powershell
# Unhide and create the plan
powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61

# If blocked by Modern Standby, override first:
New-ItemProperty -Path "HKLM:\System\CurrentControlSet\Control\Power" -Name "PlatformAoAcOverride" -Value 0 -PropertyType DWord -Force
# Then reboot and re-run the duplicatescheme command

# Activate (parse GUID from output)
$plan = powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61
$guid = ($plan -split ":\s*" | Select-Object -Last 1).Trim()
powercfg -setactive $guid
```

Fallback to High Performance if Ultimate Performance fails: GUID `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`.

**9. Disable Power Throttling** — Impact: **Medium**

Prevents Windows from reducing CPU frequency for background processes during gaming. Primarily relevant on laptops but also affects desktops.

```powershell
$path = "HKLM:\SYSTEM\CurrentControlSet\Control\Power\PowerThrottling"
if (-not (Test-Path $path)) { New-Item -Path $path -Force }
New-ItemProperty -Path $path -Name "PowerThrottlingOff" -Value 1 -PropertyType DWord -Force
```

**10. Timer Resolution — GlobalTimerResolutionRequests** — Impact: **High**

This is one of the few tweaks with **proven, measurable impact**. Windows 11 changed behavior so minimized/occluded windows no longer receive high timer resolution. This registry key restores global timer resolution requests, improving frame pacing and reducing input latency — especially at **240 Hz+** refresh rates.

```
HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\kernel
  GlobalTimerResolutionRequests = 1 (DWORD)
```

The jump from the default 15.625 ms timer to 1 ms is measurable with PresentMon/CapFrameX: tighter frame times and **20–30% improvement in 1%/0.1% lows** are documented by the Blur Busters community. This key is specific to Windows 11 and Windows Server 21H2+.

**11. SvcHost Split Threshold** — Impact: **Low**

Consolidates svchost.exe processes based on available RAM, reducing process count and context switching overhead. WinUtil dynamically sets this based on system memory.

```powershell
$ram = (Get-CimInstance Win32_PhysicalMemory | Measure-Object -Property Capacity -Sum).Sum / 1KB
Set-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control" -Name "SvcHostSplitThresholdInKB" -Value $ram -Type DWord
```

**12. MMCSS Gaming Task Priority** — Impact: **Low** (unproven by rigorous benchmarks)

The Multimedia Class Scheduler Service allows registering a "Games" task with elevated priority. Widely referenced but no reputable outlet has benchmarked it.

```
HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile
  SystemResponsiveness = 0 (DWORD)  # Reserve 0% CPU for background tasks

HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games
  GPU Priority = 8 (DWORD)
  Priority = 6 (DWORD)
  Scheduling Category = "High" (REG_SZ)
  SFIO Priority = "High" (REG_SZ)
```

---

## Network optimizations: TCP tuning and throttling

**13. Disable Nagle's Algorithm** — Impact: **Low** (plausible for TCP-only games)

Most modern multiplayer games use **UDP, not TCP**, making this tweak irrelevant for them. Games that use TCP (some MMOs, League of Legends) may see 10–20 ms latency reduction. Well-coded games already set `TCP_NODELAY` on their sockets programmatically.

```powershell
# Must be applied to the active NIC's interface GUID
# Find it: Get-NetAdapter | Select-Object Name, InterfaceGuid
$nicGuid = (Get-NetAdapter | Where-Object Status -eq "Up" | Select-Object -First 1).InterfaceGuid
$tcpPath = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$nicGuid"
New-ItemProperty -Path $tcpPath -Name "TcpAckFrequency" -Value 1 -PropertyType DWord -Force
New-ItemProperty -Path $tcpPath -Name "TCPNoDelay" -Value 1 -PropertyType DWord -Force
```

**14. Disable Network Throttling Index** — Impact: **Snake Oil**

This Vista-era throttle limits network packet processing to ~10 packets/ms to prioritize multimedia playback. On modern multi-core CPUs with high-speed NICs, this throttle is never the bottleneck. No credible benchmark from Hardware Unboxed, GamersNexus, or Digital Foundry has shown measurable improvement. The absence of coverage from these outlets is itself telling.

```
HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile
  NetworkThrottlingIndex = 0xFFFFFFFF (DWORD)
```

Include if desired for completeness — the tweak is harmless — but label honestly.

**15. Disable TCP Auto-Tuning** — Impact: **Low** (situational)

Can help on networks with buggy routers or middleboxes that mishandle TCP window scaling. Generally harmful on modern, well-configured networks because it prevents optimal window sizing.

```powershell
netsh int tcp set global autotuninglevel=disabled
# Restore: netsh int tcp set global autotuninglevel=normal
```

---

## Storage optimizations

**16. Disable NTFS Last Access Timestamp** — Impact: **Low**

Reduces write operations per file access on NTFS volumes. On Windows 11, volumes >128 GB already have this disabled by default ("System Managed, Disabled"). Explicitly setting it ensures it's off regardless of volume size.

```powershell
fsutil behavior set disablelastaccess 1
```

Registry equivalent:
```
HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem
  NtfsDisableLastAccessUpdate = 0x80000001 (DWORD)
```

Query current state: `fsutil behavior query disablelastaccess`

**17. Disable Hibernation** — Impact: **Medium**

Frees **8–16 GB** of disk space (hiberfil.sys) and disables Fast Startup, which can cause driver/state issues after "shutdown."

```powershell
powercfg /h off
```

---

## UI/UX and background process reduction

**18. Disable Background Apps (Global)** — Impact: **Medium**

Prevents all UWP apps from running in the background. From WinUtil source code.

```powershell
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\BackgroundAccessApplications" -Name "GlobalUserDisabled" -Value 1 -Type DWord -Force
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Search" -Name "BackgroundAppGlobalToggle" -Value 0 -Type DWord -Force
```

**19. Disable Copilot / Cortana** — Impact: **Medium**

Removes background AI assistant processes. Copilot's resource footprint has grown significantly since its integration into Windows 11 24H2.

```powershell
# Disable Copilot
New-Item -Path "HKCU:\Software\Policies\Microsoft\Windows\WindowsCopilot" -Force
New-ItemProperty -Path "HKCU:\Software\Policies\Microsoft\Windows\WindowsCopilot" -Name "TurnOffWindowsCopilot" -Value 1 -PropertyType DWord -Force
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced" -Name "ShowCopilotButton" -Value 0 -Type DWord -Force

# Uninstall Copilot app
Get-AppxPackage -AllUsers | Where-Object Name -ilike "*Copilot*" | Remove-AppxPackage -AllUsers -ErrorAction SilentlyContinue

# Disable Cortana
New-Item -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search" -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\Windows Search" -Name "AllowCortana" -Value 0 -PropertyType DWord -Force
```

**20. Disable Windows Tips, Suggestions, and Bloatware Auto-Install** — Impact: **Medium**

The ContentDeliveryManager is responsible for silently installing promoted apps (Candy Crush, TikTok, etc.) and showing lock screen/Start suggestions. Disabling it prevents unwanted background downloads and installations.

```powershell
$cdm = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\ContentDeliveryManager"
$values = @{
    "ContentDeliveryAllowed" = 0
    "OemPreInstalledAppsEnabled" = 0
    "PreInstalledAppsEnabled" = 0
    "PreInstalledAppsEverEnabled" = 0
    "SilentInstalledAppsEnabled" = 0
    "SoftLandingEnabled" = 0
    "SubscribedContentEnabled" = 0
    "SubscribedContent-310093Enabled" = 0
    "SubscribedContent-338388Enabled" = 0
    "SubscribedContent-338389Enabled" = 0
    "SubscribedContent-338393Enabled" = 0
    "SubscribedContent-353694Enabled" = 0
    "SubscribedContent-353696Enabled" = 0
    "SystemPaneSuggestionsEnabled" = 0
}
foreach ($key in $values.Keys) {
    Set-ItemProperty -Path $cdm -Name $key -Value $values[$key] -Type DWord -Force
}

# Machine-level policy to prevent consumer feature downloads
New-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent" -Name "DisableWindowsConsumerFeatures" -Value 1 -PropertyType DWord -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent" -Name "DisableConsumerAccountStateContent" -Value 1 -PropertyType DWord -Force
New-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\PushToInstall" -Name "DisablePushToInstall" -Value 1 -PropertyType DWord -Force
```

**21. Disable Windows Telemetry (Registry)** — Impact: **Medium**

The core telemetry registry keys, complementing the service-level disable.

```powershell
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo" -Name "Enabled" -Value 0 -Type DWord
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Privacy" -Name "TailoredExperiencesWithDiagnosticDataEnabled" -Value 0 -Type DWord
New-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection" -Name "AllowTelemetry" -Value 0 -PropertyType DWord -Force
```

**22. Disable Mouse Acceleration** — Impact: **High** (for FPS gaming)

Critical for consistent aim in FPS games. Removes the non-linear mouse response curve.

```powershell
Set-ItemProperty -Path "HKCU:\Control Panel\Mouse" -Name "MouseSpeed" -Value "0"
Set-ItemProperty -Path "HKCU:\Control Panel\Mouse" -Name "MouseThreshold1" -Value "0"
Set-ItemProperty -Path "HKCU:\Control Panel\Mouse" -Name "MouseThreshold2" -Value "0"
```

**23. Disable Sticky Keys Shortcut** — Impact: **Low** (QoL)

Prevents accidental activation of Sticky Keys (5x Shift) during gaming sessions.

**24. Disable Bing Search in Start Menu** — Impact: **Low**

```
HKCU:\Software\Microsoft\Windows\CurrentVersion\Search
  BingSearchEnabled = 0 (DWORD)
```

---

## Services and scheduled tasks to disable

**25. Safe Services to Disable** — Impact: **Medium** (collectively saves 100–300 MB RAM and reduces CPU spikes)

FrameGuard should offer these as a curated list with checkboxes, grouped by category. Services marked with ⚠️ should include a conditional warning.

```powershell
# Telemetry
Set-Service -Name "DiagTrack" -StartupType Disabled          # Connected User Experiences & Telemetry
Set-Service -Name "dmwappushservice" -StartupType Disabled    # WAP Push Message Routing

# Diagnostics
Set-Service -Name "diagsvc" -StartupType Disabled            # Diagnostic Execution Service
Set-Service -Name "WerSvc" -StartupType Disabled             # Windows Error Reporting

# Compatibility
Set-Service -Name "PcaSvc" -StartupType Disabled             # Program Compatibility Assistant

# Unused hardware (conditional)
Set-Service -Name "Fax" -StartupType Disabled                # Fax
Set-Service -Name "Spooler" -StartupType Disabled            # ⚠️ Print Spooler (only if no printer)
Set-Service -Name "lfsvc" -StartupType Disabled              # Geolocation Service
Set-Service -Name "MapsBroker" -StartupType Disabled         # Downloaded Maps Manager
Set-Service -Name "PhoneSvc" -StartupType Disabled           # Phone Service
Set-Service -Name "WbioSrvc" -StartupType Disabled           # ⚠️ Windows Biometric (if no fingerprint)
Set-Service -Name "bthserv" -StartupType Disabled            # ⚠️ Bluetooth (if no BT devices)
Set-Service -Name "SensorDataService" -StartupType Disabled  # Sensor Data Service
Set-Service -Name "SensrSvc" -StartupType Disabled           # Sensor Monitoring Service
Set-Service -Name "SensorService" -StartupType Disabled      # Sensor Service

# Remote access
Set-Service -Name "TermService" -StartupType Disabled        # Remote Desktop Services
Set-Service -Name "RemoteRegistry" -StartupType Disabled     # Remote Registry
Set-Service -Name "RemoteAccess" -StartupType Disabled       # Routing and Remote Access

# Enterprise / unused
Set-Service -Name "wisvc" -StartupType Disabled              # Windows Insider Service
Set-Service -Name "RetailDemo" -StartupType Disabled         # Retail Demo Service
Set-Service -Name "WpcMonSvc" -StartupType Disabled          # Parental Controls
Set-Service -Name "SEMgrSvc" -StartupType Disabled           # Payments and NFC/SE Manager
Set-Service -Name "AJRouter" -StartupType Disabled           # AllJoyn Router Service
Set-Service -Name "WalletService" -StartupType Disabled      # Wallet Service
Set-Service -Name "ScDeviceEnum" -StartupType Disabled       # Smart Card Device Enumeration
Set-Service -Name "SCardSvr" -StartupType Disabled           # Smart Card
Set-Service -Name "SCPolicySvc" -StartupType Disabled        # Smart Card Removal Policy

# Set to Manual (delayed start) instead of Automatic
Set-Service -Name "WSearch" -StartupType Manual              # Windows Search Indexer
Set-Service -Name "BITS" -StartupType Manual                 # Background Intelligent Transfer
```

**Do NOT disable**: `AudioSrv`, `AudioEndpointBuilder`, `DispBrokerDesktopSvc`, `SysMain`, `wuauserv`, `Dnscache`, `Dhcp`, `mpssvc`, `CryptSvc`, `Winmgmt`, `PlugPlay`, `GraphicsPerfSvc`.

**26. Scheduled Tasks to Disable** — Impact: **Medium** (eliminates periodic CPU/disk spikes from telemetry)

The **Microsoft Compatibility Appraiser** is the single most impactful task to disable — it runs `CompatTelRunner.exe`, which causes documented CPU spikes of 10–30% lasting several minutes.

```powershell
$tasks = @(
    # Telemetry (highest impact)
    "\Microsoft\Windows\Application Experience\Microsoft Compatibility Appraiser",
    "\Microsoft\Windows\Application Experience\ProgramDataUpdater",
    "\Microsoft\Windows\Application Experience\StartupAppTask",
    
    # CEIP
    "\Microsoft\Windows\Customer Experience Improvement Program\Consolidator",
    "\Microsoft\Windows\Customer Experience Improvement Program\UsbCeip",
    "\Microsoft\Windows\Customer Experience Improvement Program\KernelCeipTask",
    
    # Diagnostics
    "\Microsoft\Windows\DiskDiagnostic\Microsoft-Windows-DiskDiagnosticDataCollector",
    "\Microsoft\Windows\Feedback\Siuf\DmClient",
    "\Microsoft\Windows\Feedback\Siuf\DmClientOnScenarioDownload",
    "\Microsoft\Windows\Windows Error Reporting\QueueReporting",
    "\Microsoft\Windows\Autochk\Proxy",
    "\Microsoft\Windows\DiskFootprint\Diagnostics",
    "\Microsoft\Windows\Power Efficiency Diagnostics\AnalyzeSystem",
    
    # Family/unused
    "\Microsoft\Windows\Shell\FamilySafetyMonitor",
    "\Microsoft\Windows\Shell\FamilySafetyRefreshTask"
)

foreach ($task in $tasks) {
    Disable-ScheduledTask -TaskName $task -ErrorAction SilentlyContinue
}
```

---

## The snake oil filter: what to avoid or label honestly

Every optimization tool has credibility at stake. FrameGuard should clearly distinguish proven tweaks from placebo. The table below summarizes evidence quality for commonly recommended tweaks.

| Tweak | Rating | Reasoning |
|---|---|---|
| Disable Game DVR recording | **Proven** | Measurable 1–3% CPU savings; confirmed by multiple sources |
| Timer resolution (15.6 ms → 1 ms) | **Proven** | Measurable frame pacing improvement; documented by Blur Busters |
| Ultimate Performance power plan | **Proven** | Locks CPU at max P-state; measurable in CPU-bound scenarios |
| Disable telemetry tasks (Appraiser) | **Proven** | CompatTelRunner causes 10–30% CPU spikes lasting minutes |
| MSI mode for GPU | **Proven** | Measurably reduces DPC latency on pre-40-series NVIDIA cards |
| Disable Power Throttling | **Plausible** | Technical reasoning is sound; no rigorous FPS benchmarks |
| Disable background apps | **Plausible** | Reduces RAM and CPU usage; benefit depends on installed apps |
| NVIDIA DisableDynamicPstate | **Plausible** | Forces max clocks; measurable clock-locking vs NVCP "Prefer Max" |
| Nagle's algorithm disable | **Plausible** | Only affects TCP-based games; most modern games use UDP |
| Disable fullscreen optimizations | **Snake Oil** (Win11) | Was relevant on Win10; Win11's FSO is well-optimized; ±1 FPS |
| NetworkThrottlingIndex | **Snake Oil** | No credible benchmark shows improvement on modern hardware |
| IRQ priority for GPU | **Snake Oil** | Obsolete since MSI/MSI-X became standard on modern hardware |
| SystemResponsiveness = 0 | **Unproven** | Widely shared, no rigorous benchmarks from reputable outlets |
| GPU Priority = 8 in MMCSS | **Unproven** | Widely shared, no rigorous benchmarks |
| bcdedit useplatformtick yes | **Harmful** | Reports of increased input lag on Win11 |
| bcdedit useplatformclock true | **Harmful** | Consistently worsens FPS and DPC latency |
| Spectre/Meltdown disable | **Harmful** | Negligible FPS gain on modern CPUs; severe security vulnerability |
| Disable SysMain on SSD | **Obsolete** | Was relevant for HDD; negligible effect on SSD systems |
| Disable HPET in BIOS | **Already Default** | Windows already uses TSC; HPET kept only for sync |

**Fullscreen Optimizations deserves special treatment**: FrameGuard could still include this as an option with honest labeling ("legacy tweak for DX9/DX11 titles; no measured benefit on DX12/Vulkan games in Windows 11"). The WinUtil registry keys for it are:

```
HKCU:\System\GameConfigStore
  GameDVR_FSEBehaviorMode = 2
  GameDVR_HonorUserFSEBehaviorMode = 1
  GameDVR_FSEBehavior = 2
  GameDVR_DXGIHonorFSEWindowsCompatible = 1
  GameDVR_EFSEFeatureFlags = 0
```

**Spectre/Meltdown mitigations** should be included only as information with a strong red warning. The registry keys are `FeatureSettingsOverride = 3` and `FeatureSettingsOverrideMask = 3` under `HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management`. Tom's Hardware tested 10 CPUs and found **no meaningful FPS difference** on modern silicon. One user's 3DMark actually scored lower with mitigations disabled (8044 vs 8060). The security exposure to JavaScript-exploitable Spectre variants makes this an indefensible tradeoff for gaming.

---

## Conclusion: a prioritized implementation roadmap

The highest-value additions to FrameGuard fall into three tiers. **Tier 1** (implement immediately) includes Game DVR disable, Ultimate Performance power plan, GlobalTimerResolutionRequests, telemetry service/task disabling, ContentDeliveryManager bloatware prevention, Copilot/Cortana removal, and background app disabling — these have proven or strongly plausible benefits with minimal risk. **Tier 2** (implement with appropriate warnings) includes MSI mode for GPU, NVIDIA telemetry disable, Disable MPO, Power Throttling off, mouse acceleration disable, and the curated services list — these are situational but safe. **Tier 3** (include as advanced/optional with honest labeling) includes fullscreen optimizations, Nagle's algorithm, NVIDIA DisableDynamicPstate, bcdedit disabledynamictick, and NetworkThrottlingIndex.

The single most important design decision is transparency. Tools that label snake oil as "performance optimization" lose credibility with knowledgeable users. FrameGuard should rate each tweak's evidence level directly in the UI — an approach no major competitor currently takes.