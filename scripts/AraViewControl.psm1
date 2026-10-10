#requires -Version 7
<#
.SYNOPSIS
  실행 중인 AraView 창을 밖에서 확인하고 조작한다.
.DESCRIPTION
  개발(디버그) 빌드는 `\\.\pipe\<식별자>-control` 제어 통로를 연다. 이 모듈은 그 통로로
  요청 한 줄(JSON)을 보내고 응답 한 줄(JSON)을 받아, 클릭, 키, 드래그, 휠 같은 입력을
  앱의 실제 입력 경로에 넣고 상태를 읽는다. 캡처는 PrintWindow를 쓰므로 사용자의 마우스나
  키보드를 건드리지 않는다.

  설치는 필요 없다. 저장소 스크립트가 스스로 C# P/Invoke를 컴파일한다.
  사용법은 `scripts/araview-drive.ps1`(CLI)과 `scripts/araview-mcp.ps1`(MCP 서버)를 본다.
#>

Set-StrictMode -Version Latest

# 브리지가 없는 릴리스 빌드에서는 창 찾기와 캡처만 쓸 수 있다.
$script:Identifiers = @{
  dev     = 'com.araview.viewer.dev'
  release = 'com.araview.viewer'
}

if (-not ('AraViewControl.Win32' -as [type])) {
  # System.Drawing은 PowerShell 7.6에서 컴파일 참조가 깨져 쓸 수 없다. GDI로 픽셀을 받아
  # PNG를 직접 쓴다(의존성 없음).
  Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Runtime.InteropServices;
using System.Text;

namespace AraViewControl
{
    /// 캡처 결과. 픽셀은 창 전체(비클라이언트 영역 포함)다.
    public sealed class Shot
    {
        public int Width;
        public int Height;
        /// 캡처 이미지 안에서 클라이언트 영역 왼쪽 위의 위치. 입력 좌표는 클라이언트 기준이다.
        public int ClientX;
        public int ClientY;
        /// 표본 픽셀의 평균 밝기(0~255). 0에 가까우면 아무것도 그려지지 않은 캡처다.
        public double Mean;
        /// 실제로 쓴 방법: print(PrintWindow), render(PrintWindow+PW_RENDERFULLCONTENT), screen(화면 복사).
        public string Method;
        /// 최소화되어 있던 창을 복원했는지.
        public bool Restored;
        public byte[] Png;
    }

    public sealed class WindowInfo
    {
        public long Handle;
        public uint Pid;
        public string Title;
        public int Left;
        public int Top;
        public int Width;
        public int Height;
        public int ClientWidth;
        public int ClientHeight;
        /// 캡처 이미지(창 전체) 안에서 클라이언트 영역 왼쪽 위의 위치.
        public int ClientX;
        public int ClientY;
        public int Dpi;
    }

    public static class Win32
    {
        [StructLayout(LayoutKind.Sequential)]
        public struct RECT { public int Left, Top, Right, Bottom; }

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT { public int X, Y; }

        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")]
        private static extern bool IsIconic(IntPtr hWnd);
        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int command);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int max);
        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
        [DllImport("user32.dll")]
        private static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
        [DllImport("user32.dll")]
        private static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
        [DllImport("user32.dll")]
        private static extern int GetDpiForWindow(IntPtr hWnd);
        [DllImport("user32.dll")]
        private static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);

        [DllImport("user32.dll")]
        private static extern bool SetProcessDpiAwarenessContext(IntPtr context);

        /// 이 프로세스를 모니터별 DPI 인식으로 만든다. 그렇지 않으면 창 크기와 클라이언트 위치가
        /// 96dpi 기준으로 줄어 돌아오고, 125% 같은 배율에서 캡처가 오른쪽과 아래 약 20%를 잃는다.
        /// 이미 설정돼 있으면 실패하지만 그때는 이미 인식 상태이므로 무시한다.
        public static void EnableDpiAwareness()
        {
            try { SetProcessDpiAwarenessContext(new IntPtr(-4)); } catch (EntryPointNotFoundException) { }
        }

        [DllImport("user32.dll")]
        public static extern bool IsWindow(IntPtr hWnd);
        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern IntPtr GetDC(IntPtr hWnd);
        [DllImport("user32.dll")]
        private static extern int ReleaseDC(IntPtr hWnd, IntPtr hdc);
        [DllImport("gdi32.dll")]
        private static extern IntPtr CreateCompatibleDC(IntPtr hdc);
        [DllImport("gdi32.dll")]
        private static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int width, int height);
        [DllImport("gdi32.dll")]
        private static extern IntPtr SelectObject(IntPtr hdc, IntPtr handle);
        [DllImport("gdi32.dll")]
        private static extern bool DeleteObject(IntPtr handle);
        [DllImport("gdi32.dll")]
        private static extern bool DeleteDC(IntPtr hdc);
        [DllImport("gdi32.dll")]
        private static extern bool BitBlt(IntPtr target, int x, int y, int width, int height, IntPtr source, int sourceX, int sourceY, uint operation);
        [DllImport("gdi32.dll")]
        private static extern int GetDIBits(IntPtr hdc, IntPtr bitmap, uint start, uint lines, byte[] bits, ref BITMAPINFO info, uint usage);

        [StructLayout(LayoutKind.Sequential)]
        private struct BITMAPINFOHEADER
        {
            public uint Size;
            public int Width;
            public int Height;
            public ushort Planes;
            public ushort BitCount;
            public uint Compression;
            public uint SizeImage;
            public int XPelsPerMeter;
            public int YPelsPerMeter;
            public uint ClrUsed;
            public uint ClrImportant;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BITMAPINFO
        {
            public BITMAPINFOHEADER Header;
            public uint Color;
        }

        private const int SW_RESTORE = 9;
        private const uint PW_RENDERFULLCONTENT = 2;
        private const uint SRCCOPY = 0x00CC0020;

        public static WindowInfo Describe(IntPtr hwnd)
        {
            RECT frame, client;
            GetWindowRect(hwnd, out frame);
            GetClientRect(hwnd, out client);
            var title = new StringBuilder(512);
            GetWindowTextW(hwnd, title, title.Capacity);
            var point = new POINT();
            ClientToScreen(hwnd, ref point);
            uint pid;
            GetWindowThreadProcessId(hwnd, out pid);
            return new WindowInfo
            {
                Handle = hwnd.ToInt64(),
                Pid = pid,
                Title = title.ToString(),
                Left = frame.Left,
                Top = frame.Top,
                Width = frame.Right - frame.Left,
                Height = frame.Bottom - frame.Top,
                ClientWidth = client.Right - client.Left,
                ClientHeight = client.Bottom - client.Top,
                ClientX = point.X - frame.Left,
                ClientY = point.Y - frame.Top,
                Dpi = GetDpiForWindow(hwnd),
            };
        }

        /// 주어진 프로세스들이 가진 보이는 최상위 창을 모두 돌려준다.
        public static WindowInfo[] WindowsForPids(uint[] pids)
        {
            var wanted = new HashSet<uint>(pids);
            var found = new List<WindowInfo>();
            EnumWindows((hwnd, _) =>
            {
                uint pid;
                GetWindowThreadProcessId(hwnd, out pid);
                if (!wanted.Contains(pid) || !IsWindowVisible(hwnd)) return true;
                var info = Describe(hwnd);
                if (info.Width <= 0 || info.Height <= 0) return true;
                found.Add(info);
                return true;
            }, IntPtr.Zero);
            return found.ToArray();
        }

        /// 창을 캡처한다. method: 0=자동, 1=PrintWindow, 2=PrintWindow+PW_RENDERFULLCONTENT, 3=화면 복사.
        public static Shot Capture(long handle, int method)
        {
            var hwnd = new IntPtr(handle);
            if (!IsWindow(hwnd)) throw new InvalidOperationException("window is gone: " + handle);
            bool restored = false;
            // 최소화된 창은 PrintWindow가 빈 그림을 준다. 상태를 바꾸는 일이라 결과에 남긴다.
            if (IsIconic(hwnd)) { ShowWindow(hwnd, SW_RESTORE); restored = true; System.Threading.Thread.Sleep(120); }

            int[] order = method == 0 ? new[] { 2, 1, 3 } : new[] { method };
            Shot best = null;
            foreach (int candidate in order)
            {
                var shot = Grab(hwnd, candidate);
                shot.Restored = restored;
                if (best == null || shot.Mean > best.Mean) best = shot;
                // 내용이 보이면 더 시도하지 않는다. 자동 모드는 검은 캡처를 걸러내기 위한 것이다.
                if (shot.Mean > 1.0) return shot;
            }
            return best;
        }

        private static Shot Grab(IntPtr hwnd, int method)
        {
            RECT frame;
            if (!GetWindowRect(hwnd, out frame)) throw new InvalidOperationException("GetWindowRect failed");
            int width = frame.Right - frame.Left;
            int height = frame.Bottom - frame.Top;
            if (width <= 0 || height <= 0) throw new InvalidOperationException("window has no area");

            byte[] pixels = null;
            IntPtr screenDc = GetDC(IntPtr.Zero);
            IntPtr memoryDc = IntPtr.Zero;
            IntPtr bitmap = IntPtr.Zero;
            IntPtr previous = IntPtr.Zero;
            try
            {
                memoryDc = CreateCompatibleDC(screenDc);
                bitmap = CreateCompatibleBitmap(screenDc, width, height);
                previous = SelectObject(memoryDc, bitmap);
                if (method == 3)
                {
                    // 화면에 보이는 픽셀을 그대로 복사한다. 다른 창이 겹치면 함께 찍힌다.
                    BitBlt(memoryDc, 0, 0, width, height, screenDc, frame.Left, frame.Top, SRCCOPY);
                }
                else
                {
                    uint flags = method == 2 ? PW_RENDERFULLCONTENT : 0u;
                    PrintWindow(hwnd, memoryDc, flags);
                }
                pixels = ReadPixels(screenDc, memoryDc, bitmap, width, height);
            }
            finally
            {
                if (previous != IntPtr.Zero) SelectObject(memoryDc, previous);
                if (bitmap != IntPtr.Zero) DeleteObject(bitmap);
                if (memoryDc != IntPtr.Zero) DeleteDC(memoryDc);
                ReleaseDC(IntPtr.Zero, screenDc);
            }

            var point = new POINT();
            ClientToScreen(hwnd, ref point);
            return new Shot
            {
                Width = width,
                Height = height,
                ClientX = point.X - frame.Left,
                ClientY = point.Y - frame.Top,
                Mean = MeanBrightness(pixels),
                Method = method == 2 ? "render" : method == 1 ? "print" : "screen",
                Png = EncodePng(pixels, width, height),
            };
        }

        /// BGRA 픽셀을 위에서 아래 순서로 읽는다.
        private static byte[] ReadPixels(IntPtr hdc, IntPtr memoryDc, IntPtr bitmap, int width, int height)
        {
            var info = new BITMAPINFO();
            info.Header.Size = (uint)Marshal.SizeOf(typeof(BITMAPINFOHEADER));
            info.Header.Width = width;
            info.Header.Height = -height; // 음수 = 위에서 아래
            info.Header.Planes = 1;
            info.Header.BitCount = 32;
            info.Header.Compression = 0; // BI_RGB
            var pixels = new byte[width * height * 4];
            if (GetDIBits(hdc, bitmap, 0, (uint)height, pixels, ref info, 0) == 0)
            {
                throw new InvalidOperationException("GetDIBits failed");
            }
            return pixels;
        }

        /// 표본 픽셀의 평균 밝기. 검은 캡처인지 빠르게 판단하는 데만 쓴다.
        private static double MeanBrightness(byte[] pixels)
        {
            long sum = 0;
            long count = 0;
            for (int index = 0; index < pixels.Length; index += 4)
            {
                sum += pixels[index] + pixels[index + 1] + pixels[index + 2];
                count += 3;
            }
            return count == 0 ? 0 : sum / (double)count;
        }

        /// BGRA 픽셀을 PNG(truecolor)로 쓴다. 외부 라이브러리를 쓰지 않는다.
        private static byte[] EncodePng(byte[] pixels, int width, int height)
        {
            int rowBytes = width * 3;
            var raw = new byte[(rowBytes + 1) * height];
            for (int y = 0; y < height; y++)
            {
                int source = y * width * 4;
                int target = y * (rowBytes + 1);
                raw[target] = 0; // 필터 없음
                for (int x = 0; x < width; x++)
                {
                    raw[target + 1 + x * 3] = pixels[source + x * 4 + 2];
                    raw[target + 2 + x * 3] = pixels[source + x * 4 + 1];
                    raw[target + 3 + x * 3] = pixels[source + x * 4];
                }
            }

            byte[] compressed;
            using (var stream = new MemoryStream())
            {
                stream.WriteByte(0x78); // zlib 헤더
                stream.WriteByte(0x01);
                using (var deflate = new DeflateStream(stream, CompressionLevel.Optimal, true))
                {
                    deflate.Write(raw, 0, raw.Length);
                }
                uint adler = Adler32(raw);
                stream.WriteByte((byte)(adler >> 24));
                stream.WriteByte((byte)(adler >> 16));
                stream.WriteByte((byte)(adler >> 8));
                stream.WriteByte((byte)adler);
                compressed = stream.ToArray();
            }

            using (var png = new MemoryStream())
            {
                png.Write(new byte[] { 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A }, 0, 8);
                var header = new byte[13];
                WriteInt(header, 0, width);
                WriteInt(header, 4, height);
                header[8] = 8;  // 비트 깊이
                header[9] = 2;  // truecolor
                WriteChunk(png, "IHDR", header);
                WriteChunk(png, "IDAT", compressed);
                WriteChunk(png, "IEND", new byte[0]);
                return png.ToArray();
            }
        }

        private static void WriteInt(byte[] target, int offset, int value)
        {
            target[offset] = (byte)(value >> 24);
            target[offset + 1] = (byte)(value >> 16);
            target[offset + 2] = (byte)(value >> 8);
            target[offset + 3] = (byte)value;
        }

        private static void WriteChunk(Stream stream, string type, byte[] data)
        {
            var length = new byte[4];
            WriteInt(length, 0, data.Length);
            stream.Write(length, 0, 4);
            var typeBytes = Encoding.ASCII.GetBytes(type);
            stream.Write(typeBytes, 0, 4);
            stream.Write(data, 0, data.Length);
            uint crc = Crc32Update(0xFFFFFFFF, typeBytes);
            crc = Crc32Update(crc, data) ^ 0xFFFFFFFF;
            var checksum = new byte[4];
            WriteInt(checksum, 0, unchecked((int)crc));
            stream.Write(checksum, 0, 4);
        }

        private static uint Adler32(byte[] data)
        {
            uint a = 1, b = 0;
            foreach (byte value in data)
            {
                a = (a + value) % 65521;
                b = (b + a) % 65521;
            }
            return (b << 16) | a;
        }

        private static uint Crc32Update(uint crc, byte[] data)
        {
            foreach (byte value in data)
            {
                crc ^= value;
                for (int bit = 0; bit < 8; bit++)
                {
                    crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xEDB88320u : crc >> 1;
                }
            }
            return crc;
        }
    }
}
'@
}

# 창 좌표와 캡처 크기가 실제 물리 px와 맞도록 가장 먼저 DPI 인식을 켠다.
if ([AraViewControl.Win32].GetMethod('EnableDpiAwareness')) { [AraViewControl.Win32]::EnableDpiAwareness() }

# ----- 내부 도우미 -----

function Get-AraViewIdentifier {
  param([ValidateSet('dev', 'release')][string]$Identity = 'dev')
  $script:Identifiers[$Identity]
}

function Invoke-AraViewRequest {
  <#
  .SYNOPSIS
    제어 통로에 요청 하나를 보내고 응답을 돌려준다.
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][hashtable]$Request,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$TimeoutMs = 15000
  )

  $name = "$(Get-AraViewIdentifier $Identity)-control"
  $pipe = [System.IO.Pipes.NamedPipeClientStream]::new('.', $name, [System.IO.Pipes.PipeDirection]::InOut)
  try {
    $pipe.Connect($TimeoutMs)
  } catch {
    $pipe.Dispose()
    throw "제어 통로에 연결하지 못했습니다($name). $($_.Exception.Message) 개발 빌드가 실행 중인지 확인하세요(scripts/araview-drive.ps1 launch)."
  }

  try {
    $encoding = [System.Text.UTF8Encoding]::new($false)
    $writer = [System.IO.StreamWriter]::new($pipe, $encoding, 4096, $true)
    $writer.NewLine = "`n"
    $writer.AutoFlush = $true
    $writer.WriteLine(($Request | ConvertTo-Json -Compress -Depth 10))
    $reader = [System.IO.StreamReader]::new($pipe, $encoding, $false, 4096, $true)
    $line = $reader.ReadLine()
    $reader.Dispose()
    $writer.Dispose()
  } finally {
    $pipe.Dispose()
  }

  if ([string]::IsNullOrWhiteSpace($line)) {
    throw "제어 통로가 빈 응답을 돌려주었습니다."
  }
  return ($line | ConvertFrom-Json)
}

function Assert-AraViewReply {
  param($Reply, [string]$Action)
  if (-not $Reply.ok) {
    throw "$Action 실패: $($Reply.error)"
  }
  return $Reply
}

# ----- 창 찾기 -----

function Get-AraViewWindow {
  <#
  .SYNOPSIS
    실행 중인 AraView 창을 돌려준다.
  .PARAMETER Identity
    dev(개발 빌드, 기본), release(설치 빌드), any(둘 다).
  #>
  [CmdletBinding()]
  param([ValidateSet('dev', 'release', 'any')][string]$Identity = 'dev')

  $processes = @(Get-Process -Name araview -ErrorAction SilentlyContinue)
  if ($processes.Count -eq 0) { return @() }

  $result = [System.Collections.Generic.List[object]]::new()
  foreach ($process in $processes) {
    # 설치 빌드와 개발 빌드는 식별자가 달라 경로로 구분한다.
    $path = try { $process.Path } catch { $null }
    $isDev = $path -like '*\target\debug\*' -or $path -like '*\target\release\*'
    foreach ($window in [AraViewControl.Win32]::WindowsForPids([uint32[]]@($process.Id))) {
      $result.Add([pscustomobject]@{
          Handle      = $window.Handle
          Pid         = [int]$window.Pid
          Title       = $window.Title
          Identity    = if ($isDev) { 'dev' } else { 'release' }
          Left        = $window.Left
          Top         = $window.Top
          Width       = $window.Width
          Height      = $window.Height
          ClientWidth = $window.ClientWidth
          ClientHeight = $window.ClientHeight
          ClientX     = $window.ClientX
          ClientY     = $window.ClientY
          Scale       = [math]::Round($window.Dpi / 96.0, 4)
          ExePath     = $path
        })
    }
  }

  if ($Identity -ne 'any') {
    $result = [System.Collections.Generic.List[object]]@($result | Where-Object Identity -eq $Identity)
  }
  return @($result)
}

function Get-AraViewTarget {
  <#
  .SYNOPSIS
    조작 대상 창 하나를 고른다. 제어 통로가 열린 창을 먼저 고른다.
  #>
  [CmdletBinding()]
  param([ValidateSet('dev', 'release', 'any')][string]$Identity = 'dev')

  $windows = @(Get-AraViewWindow -Identity $Identity)
  if ($windows.Count -eq 0) {
    throw "AraView 창을 찾지 못했습니다(identity=$Identity). 개발 빌드는 scripts/araview-drive.ps1 launch 로 띄웁니다."
  }
  if ($windows.Count -eq 1) { return $windows[0] }
  # 캡처만 되는 창보다 조작할 수 있는 창을 고른다.
  return $windows | Sort-Object -Property @{ Expression = { if ($_.Identity -eq 'dev') { 0 } else { 1 } } }, @{ Expression = { $_.Width * $_.Height }; Descending = $true } | Select-Object -First 1
}

# ----- 상태와 입력 -----

function Get-AraViewState {
  <#
  .SYNOPSIS
    창의 상태를 돌려준다(열린 파일, 인덱스, 배율, 패널, 창 크기 등).
  #>
  [CmdletBinding()]
  param([ValidateSet('dev', 'release')][string]$Identity = 'dev')

  $reply = Invoke-AraViewRequest -Request @{ cmd = 'state' } -Identity $Identity
  Assert-AraViewReply $reply 'state' | Out-Null
  return $reply.state
}

function Send-AraViewKey {
  <#
  .SYNOPSIS
    단축키 하나를 창에 보낸다. 예: right, enter, ctrl-shift-c.
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][string]$Spec,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 0
  )
  $reply = Invoke-AraViewRequest -Request @{ cmd = 'key'; spec = $Spec; waitMs = $WaitMs } -Identity $Identity
  return (Assert-AraViewReply $reply "key $Spec").state
}

function Send-AraViewText {
  <#
  .SYNOPSIS
    글자를 입력 상자에 넣는다(포커스가 있는 입력으로 간다).
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][string]$Value,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 0
  )
  $reply = Invoke-AraViewRequest -Request @{ cmd = 'text'; value = $Value; waitMs = $WaitMs } -Identity $Identity
  return (Assert-AraViewReply $reply "text").state
}

function ConvertTo-AraViewPoint {
  <#
  .SYNOPSIS
    캡처 이미지 픽셀 좌표를 창 논리 px로 옮긴다. -Logical이면 그대로 둔다.
  #>
  param(
    [double]$X,
    [double]$Y,
    [switch]$Logical,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev'
  )

  if ($Logical.IsPresent) { return @{ x = $X; y = $Y; space = 'logical' } }
  # 캡처 이미지는 창 테두리를 포함한다. 테두리만큼 빼고 배율로 나눈다.
  $target = Get-AraViewTarget -Identity $Identity
  $scale = if ($target.Scale -gt 0) { [double]$target.Scale } else { 1.0 }
  return @{
    x     = ($X - $target.ClientX) / $scale
    y     = ($Y - $target.ClientY) / $scale
    space = 'logical'
  }
}

function Send-AraViewMouse {
  <#
  .SYNOPSIS
    마우스 입력을 보낸다. 기본 좌표는 캡처 이미지 픽셀이고, -Logical이면 창의 논리 px다.
  .DESCRIPTION
    캡처 이미지는 창 테두리를 포함하므로, 기본 좌표는 창 테두리와 배율을 보정해 논리 px로
    옮긴 뒤 앱에 넣는다. 그래서 스크린샷에서 잰 픽셀을 그대로 쓸 수 있다.
  .PARAMETER Kind
    click, down, up, move, drag, wheel
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][ValidateSet('click', 'down', 'up', 'move', 'drag', 'wheel')][string]$Kind,
    [double]$X,
    [double]$Y,
    [double]$ToX,
    [double]$ToY,
    [ValidateSet('left', 'right', 'middle')][string]$Button = 'left',
    [int]$Count = 1,
    [int]$Steps = 8,
    [double]$Lines = 0,
    [double]$Delta = 0,
    [string[]]$Modifiers = @(),
    [switch]$Logical,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 0
  )

  $start = ConvertTo-AraViewPoint -X $X -Y $Y -Logical:$Logical -Identity $Identity
  $request = @{
    cmd = 'mouse'
    kind = $Kind
    x = $start.x
    y = $start.y
    button = $Button
    waitMs = $WaitMs
  }
  if ($start.space) { $request.space = $start.space }
  if ($Kind -eq 'drag') {
    $end = ConvertTo-AraViewPoint -X $ToX -Y $ToY -Logical:$Logical -Identity $Identity
    $request.toX = $end.x
    $request.toY = $end.y
  }
  # 빈 배열은 PowerShell이 $null로 넘길 수 있다(함수 반환이 파이프라인을 지나며 사라진다).
  if ($null -ne $Modifiers -and $Modifiers.Count -gt 0) { $request.modifiers = [string[]]$Modifiers }
  switch ($Kind) {
    'click' { $request.count = $Count }
    'drag' { $request.steps = $Steps }
    'wheel' {
      if ($Lines -ne 0) { $request.lines = $Lines }
      elseif ($Delta -ne 0) { $request.delta = $Delta }
      else { throw "wheel에는 -Lines 또는 -Delta가 필요합니다." }
    }
  }
  $reply = Invoke-AraViewRequest -Request $request -Identity $Identity
  return (Assert-AraViewReply $reply "mouse $Kind").state
}

function Send-AraViewDrop {
  <#
  .SYNOPSIS
    파일을 창에 끌어다 놓은 것처럼 연다(드롭으로 열기 경로 확인).
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][string[]]$Paths,
    [double]$X = 200,
    [double]$Y = 200,
    [switch]$Logical,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 400
  )

  $full = [string[]]@($Paths | ForEach-Object { (Resolve-Path -LiteralPath $_).Path })
  $point = ConvertTo-AraViewPoint -X $X -Y $Y -Logical:$Logical -Identity $Identity
  $request = @{
    cmd    = 'drop'
    paths  = $full
    x      = $point.x
    y      = $point.y
    space  = $point.space
    waitMs = $WaitMs
  }
  $reply = Invoke-AraViewRequest -Request $request -Identity $Identity
  return (Assert-AraViewReply $reply 'drop').state
}

function Send-AraViewAction {
  <#
  .SYNOPSIS
    뷰어 동작 하나를 실행한다(`src-gpui/src/app/menu.rs`의 동작 ID).
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][string]$Id,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 0
  )
  $reply = Invoke-AraViewRequest -Request @{ cmd = 'action'; id = $Id; waitMs = $WaitMs } -Identity $Identity
  return (Assert-AraViewReply $reply "action $Id").state
}

function Set-AraViewWindowSize {
  <#
  .SYNOPSIS
    창의 내용 크기를 논리 px로 바꾼다. 한쪽만 주면 다른 쪽은 그대로 둔다.
    플랫폼이 비동기로 바꾸므로 기본 300ms 기다린 뒤 상태(`window`)를 돌려준다.
    최대화나 전체화면에서는 앱이 거절한다. 실제 크기는 OS가 보정할 수 있으니 `window`로 읽는다.
  #>
  [CmdletBinding()]
  param(
    [Nullable[double]]$Width = $null,
    [Nullable[double]]$Height = $null,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [ValidateRange(0, 10000)][int]$WaitMs = 300
  )
  if ($null -eq $Width -and $null -eq $Height) { throw '너비나 높이 중 하나는 필요합니다.' }
  $request = @{ cmd = 'resize'; waitMs = $WaitMs }
  if ($null -ne $Width) { $request.width = [double]$Width }
  if ($null -ne $Height) { $request.height = [double]$Height }
  $reply = Invoke-AraViewRequest -Request $request -Identity $Identity
  return (Assert-AraViewReply $reply 'resize').state
}

function Open-AraViewPath {
  <#
  .SYNOPSIS
    파일이나 폴더를 실행 중인 창에서 연다.
  #>
  [CmdletBinding()]
  param(
    [Parameter(Mandatory)][string]$Path,
    [ValidateSet('dev', 'release')][string]$Identity = 'dev',
    [int]$WaitMs = 400
  )
  $full = (Resolve-Path -LiteralPath $Path).Path
  $reply = Invoke-AraViewRequest -Request @{ cmd = 'open'; path = $full; waitMs = $WaitMs } -Identity $Identity
  return (Assert-AraViewReply $reply "open $full").state
}

function Enable-AraViewWindow {
  <#
  .SYNOPSIS
    창을 앞으로 가져온다(캡처 방식이 화면 복사일 때만 필요하다).
  #>
  [CmdletBinding()]
  param([ValidateSet('dev', 'release', 'any')][string]$Identity = 'dev')
  $target = Get-AraViewTarget -Identity $Identity
  [AraViewControl.Win32]::SetForegroundWindow([IntPtr]$target.Handle) | Out-Null
  Invoke-AraViewRequest -Request @{ cmd = 'activate' } -Identity $target.Identity | Out-Null
  return $target
}

# ----- 캡처 -----

function Get-AraViewShot {
  <#
  .SYNOPSIS
    창을 캡처한다. 최소화된 창은 복원한 뒤 찍는다.
  .PARAMETER Method
    auto(기본), render, print, screen. auto는 검은 캡처를 만나면 다음 방법을 시도한다.
  .PARAMETER Out
    PNG로 저장할 경로.
  #>
  [CmdletBinding()]
  param(
    [ValidateSet('dev', 'release', 'any')][string]$Identity = 'dev',
    [long]$Handle = 0,
    [ValidateSet('auto', 'render', 'print', 'screen')][string]$Method = 'auto',
    [string]$Out
  )

  if ($Handle -eq 0) {
    $target = Get-AraViewTarget -Identity $Identity
    $Handle = $target.Handle
  }
  $code = @{ auto = 0; print = 1; render = 2; screen = 3 }[$Method]
  $shot = [AraViewControl.Win32]::Capture($Handle, $code)
  if ($Out) {
    $full = [System.IO.Path]::GetFullPath($Out)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($full)) | Out-Null
    [System.IO.File]::WriteAllBytes($full, $shot.Png)
  }
  return [pscustomobject]@{
    Handle    = $Handle
    Width     = $shot.Width
    Height    = $shot.Height
    ClientX   = $shot.ClientX
    ClientY   = $shot.ClientY
    Mean      = [math]::Round($shot.Mean, 2)
    Method    = $shot.Method
    Restored  = $shot.Restored
    # 화면이 잠겨 있거나 창이 화면에 없으면 모든 방법이 검게 나온다.
    Blank     = $shot.Mean -le 1
    Bytes     = $shot.Png.Length
    Path      = if ($Out) { $full } else { $null }
    Base64    = $null
    Png       = $shot.Png
  }
}

function Wait-AraView {
  <#
  .SYNOPSIS
    창이 멈추거나 상태가 조건에 맞을 때까지 기다린다.
  .DESCRIPTION
    -Until을 주면 상태 필드가 모두 맞을 때까지, 없으면 캡처가 -StableMs 동안 그대로일 때까지 기다린다.
    애니메이션처럼 계속 바뀌는 화면은 -TimeoutMs에서 멈추고 Stable=false로 돌려준다.
  .PARAMETER Until
    상태 필드와 기대값. 예: @{ name = 'sample.jpg'; index = 3 }
  #>
  [CmdletBinding()]
  param(
    [ValidateSet('dev', 'release', 'any')][string]$Identity = 'dev',
    [hashtable]$Until,
    [int]$StableMs = 300,
    [int]$TimeoutMs = 5000,
    [int]$PollMs = 100
  )

  $deadline = [datetime]::UtcNow.AddMilliseconds($TimeoutMs)
  if ($Until) {
    while ($true) {
      $state = Get-AraViewState -Identity $Identity
      $matched = $true
      foreach ($key in $Until.Keys) {
        $expected = $Until[$key]
        $property = $state.PSObject.Properties[$key]
        $actual = if ($null -ne $property) { $property.Value } else { $null }
        if ("$actual" -ne "$expected") { $matched = $false; break }
      }
      if ($matched) { return [pscustomobject]@{ Stable = $true; Timeout = $false; State = $state; Shot = $null } }
      if ([datetime]::UtcNow -gt $deadline) {
        return [pscustomobject]@{ Stable = $false; Timeout = $true; State = $state; Shot = $null }
      }
      Start-Sleep -Milliseconds $PollMs
    }
  }

  $previous = $null
  $last = $null
  while ($true) {
    $shot = Get-AraViewShot -Identity $Identity
    if ($null -ne $previous -and [System.Linq.Enumerable]::SequenceEqual([byte[]]$previous, [byte[]]$shot.Png)) {
      return [pscustomobject]@{ Stable = $true; Timeout = $false; State = $null; Shot = $shot }
    }
    $previous = $shot.Png
    $last = $shot
    if ([datetime]::UtcNow -gt $deadline) {
      return [pscustomobject]@{ Stable = $false; Timeout = $true; State = $null; Shot = $last }
    }
    Start-Sleep -Milliseconds $StableMs
  }
}

# ----- 실행 -----

function Start-AraViewApp {
  <#
  .SYNOPSIS
    개발 빌드를 실행하고 제어 통로가 열릴 때까지 기다린다.
  .DESCRIPTION
    이미 실행 중이면 앱의 단일 인스턴스 규칙에 따라 그 창에 -Path만 넘긴다.
  #>
  [CmdletBinding()]
  param(
    [string]$Path,
    [string]$Exe,
    [int]$TimeoutMs = 30000
  )

  $existing = @(Get-AraViewWindow -Identity dev)
  if ($existing.Count -gt 0) {
    if ($Path) { Open-AraViewPath -Path $Path -Identity dev | Out-Null }
    return [pscustomobject]@{ Started = $false; AlreadyRunning = $true; Pid = $existing[0].Pid; ExePath = $existing[0].ExePath }
  }

  if (-not $Exe) {
    $root = Split-Path $PSScriptRoot -Parent
    $Exe = Join-Path $root 'target\debug\araview.exe'
  }
  if (-not (Test-Path -LiteralPath $Exe)) {
    throw "개발 빌드가 없습니다: $Exe. 먼저 `cargo build -p araview-gpui`(또는 `cargo run -p araview-gpui`)를 실행하세요."
  }

  $arguments = @()
  if ($Path) { $arguments += (Resolve-Path -LiteralPath $Path).Path }
  $process = Start-Process -FilePath $Exe -ArgumentList $arguments -PassThru

  $deadline = [datetime]::UtcNow.AddMilliseconds($TimeoutMs)
  while ([datetime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 250
    try {
      Get-AraViewState -Identity dev | Out-Null
      return [pscustomobject]@{ Started = $true; AlreadyRunning = $false; Pid = $process.Id; ExePath = $Exe }
    } catch { }
  }
  throw "개발 빌드를 띄웠지만 제어 통로가 열리지 않았습니다(pid=$($process.Id)). 빌드가 최신인지 확인하세요."
}

# ----- 자체 점검 -----

function Test-AraViewControl {
  <#
  .SYNOPSIS
    실행 중인 개발 빌드를 실제로 조작해 모든 기능을 확인한다.
  .DESCRIPTION
    창 찾기, 상태 읽기, 캡처, 키, 수식키 휠, 마우스 드래그, 클릭(더블클릭으로 전체화면),
    글자 입력, 동작 실행, 파일 열기를 차례로 확인한다. 각 단계의 PASS/FAIL을 돌려준다.
  #>
  [CmdletBinding()]
  param(
    [string]$Sample,
    [int]$TimeoutMs = 30000
  )

  $results = [System.Collections.Generic.List[object]]::new()
  function Add-Result {
    param([string]$Name, [bool]$Passed, [string]$Detail)
    $results.Add([pscustomobject]@{ Name = $Name; Passed = $Passed; Detail = $Detail })
  }
  function Assert-Step {
    param([string]$Name, [bool]$Condition, [string]$Detail)
    Add-Result -Name $Name -Passed $Condition -Detail $Detail
  }

  $root = Split-Path $PSScriptRoot -Parent
  if (-not $Sample) { $Sample = Join-Path $root 'samples\sample.jpg' }

  try {
    $started = Start-AraViewApp -Path $Sample -TimeoutMs $TimeoutMs
    Assert-Step '창 찾기' ($true) "pid=$($started.Pid) alreadyRunning=$($started.AlreadyRunning)"
  } catch {
    Assert-Step '창 찾기' $false $_.Exception.Message
    return $results
  }

  $windows = @(Get-AraViewWindow -Identity dev)
  Assert-Step '창 목록' ($windows.Count -gt 0) "개발 빌드 창 $($windows.Count)개"

  Wait-AraView -Until @{ name = 'sample.jpg' } -TimeoutMs $TimeoutMs | Out-Null
  $state = Get-AraViewState
  Assert-Step '상태 읽기' ($state.kind -eq 'image' -and $state.count -gt 1) "name=$($state.name) index=$($state.index)/$($state.count)"

  # 창을 처음 띄운 직후에는 아직 그려지지 않았을 수 있어 잠시 다시 시도한다.
  $shot = Get-AraViewShot
  for ($attempt = 2; $attempt -le 4 -and $shot.Blank; $attempt++) {
    Start-Sleep -Milliseconds 400
    $shot = Get-AraViewShot
  }
  $captureDetail = "method=$($shot.Method) size=$($shot.Width)x$($shot.Height) mean=$($shot.Mean) bytes=$($shot.Bytes)"
  if ($shot.Blank) {
    $captureDetail += ' (검은 캡처: 화면이 잠겨 있거나 창이 보이지 않는 상태다. 로그인 화면이면 세션을 열고 다시 실행한다)'
  }
  Assert-Step '캡처' ($shot.Width -gt 0 -and $shot.Bytes -gt 1000 -and -not $shot.Blank) $captureDetail

  $before = (Get-AraViewState).index
  Send-AraViewKey -Spec 'right' | Out-Null
  Start-Sleep -Milliseconds 250
  $after = (Get-AraViewState).index
  Assert-Step '키(→)' ($after -eq $before + 1) "$before → $after"

  $zoom = (Get-AraViewState).zoom
  $viewport = (Get-AraViewState).viewport
  $cx = [double]$viewport.x + ([double]$viewport.width / 2)
  $cy = [double]$viewport.y + ([double]$viewport.height / 2)
  Send-AraViewMouse -Kind wheel -X $cx -Y $cy -Lines 1 -Modifiers ctrl -Logical | Out-Null
  Start-Sleep -Milliseconds 250
  $zoomed = (Get-AraViewState).zoom
  Assert-Step '수식키 휠(ctrl-휠 확대)' ([math]::Abs($zoomed - $zoom * 1.25) -lt 0.001) "$zoom → $zoomed"

  Send-AraViewMouse -Kind wheel -X $cx -Y $cy -Lines -1 -Modifiers ctrl -Logical | Out-Null
  Start-Sleep -Milliseconds 250
  $realized = (Get-AraViewState).zoom
  Assert-Step '수식키 휠(ctrl-휠 축소)' ([math]::Abs($realized - $zoom) -lt 0.001) "$zoomed → $realized"

  # 수식키 없는 휠은 배율이 아니라 화면을 넘긴다.
  $index = (Get-AraViewState).index
  Send-AraViewMouse -Kind wheel -X $cx -Y $cy -Lines -1 -Logical | Out-Null
  Start-Sleep -Milliseconds 250
  $moved = (Get-AraViewState).index
  Assert-Step '휠(다음 화면)' ($moved -eq $index + 1) "$index → $moved"
  Send-AraViewMouse -Kind wheel -X $cx -Y $cy -Lines 1 -Logical | Out-Null
  Start-Sleep -Milliseconds 250

  # 드래그 팬: 이미지가 화면보다 커질 때까지 확대하고, 그림이 올라올 때까지 기다린다.
  foreach ($_ in 1..8) { Send-AraViewAction -Id 'zoomIn' | Out-Null }
  Wait-AraView -StableMs 200 -TimeoutMs 3000 | Out-Null
  $position = (Get-AraViewState).position
  $panned = $position
  for ($attempt = 1; $attempt -le 3; $attempt++) {
    Send-AraViewMouse -Kind drag -X $cx -Y $cy -ToX ($cx + 30) -ToY ($cy + 20) -Logical | Out-Null
    Start-Sleep -Milliseconds 250
    $panned = (Get-AraViewState).position
    if ([math]::Abs([double]$panned.x - [double]$position.x) -gt 1 -or [math]::Abs([double]$panned.y - [double]$position.y) -gt 1) { break }
    # 페이지를 아직 읽는 중이면 팬이 무시된다. 잠시 기다렸다 다시 시도한다.
    Wait-AraView -StableMs 200 -TimeoutMs 3000 | Out-Null
  }
  Assert-Step '드래그 팬' ([math]::Abs([double]$panned.x - [double]$position.x) -gt 1 -or [math]::Abs([double]$panned.y - [double]$position.y) -gt 1) "($($position.x),$($position.y)) → ($($panned.x),$($panned.y)) (zoom=$((Get-AraViewState).zoom))"

  Send-AraViewAction -Id 'resetView' | Out-Null
  Start-Sleep -Milliseconds 200

  # 좌표계 두 가지를 모두 확인한다: 논리 px(창 기준)와 캡처 이미지 픽셀(테두리 포함).
  $beforeFull = (Get-AraViewState).fullscreen
  Send-AraViewMouse -Kind click -X $cx -Y $cy -Count 2 -Logical | Out-Null
  Start-Sleep -Milliseconds 400
  $afterFull = (Get-AraViewState).fullscreen
  Assert-Step '더블클릭(논리 px, 전체화면 전환)' ($afterFull -ne $beforeFull) "$beforeFull → $afterFull"
  $imageX = $cx + $shot.ClientX
  $imageY = $cy + $shot.ClientY
  Send-AraViewMouse -Kind click -X $imageX -Y $imageY -Count 2 | Out-Null
  Start-Sleep -Milliseconds 400
  Assert-Step '더블클릭(캡처 이미지 px, 복귀)' ((Get-AraViewState).fullscreen -eq $beforeFull) "image=($imageX,$imageY)"

  $info = (Get-AraViewState).infoOpen
  Send-AraViewAction -Id 'toggleExif' | Out-Null
  Start-Sleep -Milliseconds 200
  Assert-Step '동작 실행(toggleExif)' ((Get-AraViewState).infoOpen -ne $info) ""
  Send-AraViewAction -Id 'toggleExif' | Out-Null
  Start-Sleep -Milliseconds 200

  Send-AraViewAction -Id 'togglePalette' | Out-Null
  Start-Sleep -Milliseconds 300
  $dialog = (Get-AraViewState).dialogOpen
  Send-AraViewText -Value 'zoom out' | Out-Null
  Start-Sleep -Milliseconds 300
  Send-AraViewKey -Spec 'enter' | Out-Null
  Start-Sleep -Milliseconds 300
  $closed = -not (Get-AraViewState).dialogOpen
  Assert-Step '글자 입력(팔레트에서 zoom out)' ($dialog -and $closed) "dialogOpen=$dialog → $(-not $closed)"

  $other = Join-Path $root 'samples\sample.bmp'
  Open-AraViewPath -Path $other | Out-Null
  Start-Sleep -Milliseconds 400
  $opened = (Get-AraViewState).name
  Assert-Step '파일 열기' ($opened -eq 'sample.bmp') "name=$opened"

  Send-AraViewDrop -Paths @(Join-Path $root 'samples\sample.png') | Out-Null
  Start-Sleep -Milliseconds 400
  $dropped = (Get-AraViewState).name
  Assert-Step '드롭으로 열기' ($dropped -eq 'sample.png') "name=$dropped"

  Open-AraViewPath -Path $Sample | Out-Null
  return $results
}

Export-ModuleMember -Function @(
  'Get-AraViewWindow', 'Get-AraViewTarget', 'Get-AraViewState',
  'Send-AraViewKey', 'Send-AraViewText', 'Send-AraViewMouse', 'Send-AraViewDrop', 'Send-AraViewAction', 'Set-AraViewWindowSize',
  'Open-AraViewPath', 'Enable-AraViewWindow', 'Invoke-AraViewRequest',
  'Get-AraViewShot', 'Wait-AraView', 'Start-AraViewApp', 'Test-AraViewControl'
)
