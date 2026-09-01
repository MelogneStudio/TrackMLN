using System;
using System.Runtime.InteropServices;

namespace TransparencyBlurTest;

/// <summary>Actual per-pixel-alpha HWND test surface; no XAML effects or global alpha.</summary>
internal static class LayeredTestWindow
{
    private const int WindowWidth = 640, WindowHeight = 440;
    private const int RectangleWidth = 240, RectangleHeight = 180;
    private const byte BackgroundAlpha = 38; // #000000 at 38/255 = 14.902%.
    private const byte OpaqueAlpha = 255;
    private const int WS_POPUP = unchecked((int)0x80000000), WS_VISIBLE = 0x10000000, WS_EX_LAYERED = 0x00080000;
    private const uint ULW_ALPHA = 2, WM_DESTROY = 2;
    private const byte AC_SRC_OVER = 0, AC_SRC_ALPHA = 1;

    private static readonly WndProc WindowProcedure = WindowProc; // prevents delegate GC
    private static ushort _classAtom;

    public static void Show()
    {
        EnsureWindowClass();
        int left = (GetSystemMetrics(0) - WindowWidth) / 2;
        int top = (GetSystemMetrics(1) - WindowHeight) / 2;
        IntPtr hwnd = CreateWindowEx(WS_EX_LAYERED, "TransparencyBlurTest.LayeredWindow", "Transparency Test",
            WS_POPUP | WS_VISIBLE, left, top, WindowWidth, WindowHeight, IntPtr.Zero, IntPtr.Zero, GetModuleHandle(null), IntPtr.Zero);
        if (hwnd == IntPtr.Zero) ThrowLastError();
        Paint(hwnd, left, top);
    }

    private static void EnsureWindowClass()
    {
        if (_classAtom != 0) return;
        WNDCLASS windowClass = new() { lpfnWndProc = WindowProcedure, hInstance = GetModuleHandle(null), lpszClassName = "TransparencyBlurTest.LayeredWindow" };
        _classAtom = RegisterClass(ref windowClass);
        if (_classAtom == 0 && Marshal.GetLastWin32Error() != 1410) ThrowLastError(); // 1410: already registered
    }

    private static void Paint(IntPtr hwnd, int left, int top)
    {
        // UpdateLayeredWindow expects premultiplied BGRA. Every non-rectangle pixel
        // is #00000026; every rectangle pixel is #FFFFFFFF, independently opaque.
        byte[] pixels = new byte[WindowWidth * WindowHeight * 4];
        for (int pixel = 0; pixel < WindowWidth * WindowHeight; pixel++) pixels[pixel * 4 + 3] = BackgroundAlpha;
        int rectLeft = (WindowWidth - RectangleWidth) / 2;
        int rectTop = (WindowHeight - RectangleHeight) / 2;
        for (int y = rectTop; y < rectTop + RectangleHeight; y++)
        for (int x = rectLeft; x < rectLeft + RectangleWidth; x++)
        {
            int offset = (y * WindowWidth + x) * 4;
            pixels[offset] = pixels[offset + 1] = pixels[offset + 2] = pixels[offset + 3] = OpaqueAlpha;
        }

        BITMAPINFO info = new();
        info.bmiHeader.biSize = (uint)Marshal.SizeOf<BITMAPINFOHEADER>();
        info.bmiHeader.biWidth = WindowWidth;
        info.bmiHeader.biHeight = -WindowHeight; // top-down
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;

        IntPtr screenDc = GetDC(IntPtr.Zero);
        IntPtr memoryDc = CreateCompatibleDC(screenDc);
        IntPtr bitmap = CreateDIBSection(screenDc, ref info, 0, out IntPtr bits, IntPtr.Zero, 0);
        if (memoryDc == IntPtr.Zero || bitmap == IntPtr.Zero) ThrowLastError();
        IntPtr oldBitmap = SelectObject(memoryDc, bitmap);
        try
        {
            Marshal.Copy(pixels, 0, bits, pixels.Length);
            POINT destination = new(left, top), source = new(0, 0);
            SIZE size = new(WindowWidth, WindowHeight);
            BLENDFUNCTION blend = new(AC_SRC_OVER, 0, OpaqueAlpha, AC_SRC_ALPHA);
            if (!UpdateLayeredWindow(hwnd, screenDc, ref destination, ref size, memoryDc, ref source, 0, ref blend, ULW_ALPHA)) ThrowLastError();
        }
        finally
        {
            SelectObject(memoryDc, oldBitmap);
            DeleteObject(bitmap);
            DeleteDC(memoryDc);
            ReleaseDC(IntPtr.Zero, screenDc);
        }
    }

    private static IntPtr WindowProc(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam)
    {
        if (message == WM_DESTROY) PostQuitMessage(0);
        return DefWindowProc(hwnd, message, wParam, lParam);
    }

    private static void ThrowLastError() => Marshal.ThrowExceptionForHR(Marshal.GetHRForLastWin32Error());

    [StructLayout(LayoutKind.Sequential)] private struct POINT { public int X, Y; public POINT(int x, int y) => (X, Y) = (x, y); }
    [StructLayout(LayoutKind.Sequential)] private struct SIZE { public int Width, Height; public SIZE(int width, int height) => (Width, Height) = (width, height); }
    [StructLayout(LayoutKind.Sequential)] private struct BLENDFUNCTION
    {
        public byte BlendOp, BlendFlags, SourceConstantAlpha, AlphaFormat;
        public BLENDFUNCTION(byte op, byte flags, byte alpha, byte format) => (BlendOp, BlendFlags, SourceConstantAlpha, AlphaFormat) = (op, flags, alpha, format);
    }
    [StructLayout(LayoutKind.Sequential)] private struct BITMAPINFOHEADER
    {
        public uint biSize; public int biWidth, biHeight; public ushort biPlanes, biBitCount; public uint biCompression, biSizeImage;
        public int biXPelsPerMeter, biYPelsPerMeter; public uint biClrUsed, biClrImportant;
    }
    [StructLayout(LayoutKind.Sequential)] private struct BITMAPINFO { public BITMAPINFOHEADER bmiHeader; public uint bmiColors; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] private struct WNDCLASS
    {
        public uint style; public WndProc? lpfnWndProc; public int cbClsExtra, cbWndExtra; public IntPtr hInstance, hIcon, hCursor, hbrBackground;
        [MarshalAs(UnmanagedType.LPWStr)] public string? lpszMenuName;
        [MarshalAs(UnmanagedType.LPWStr)] public string? lpszClassName;
    }
    private delegate IntPtr WndProc(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern ushort RegisterClass(ref WNDCLASS windowClass);
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern IntPtr CreateWindowEx(int exStyle, string className, string windowName, int style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
    [DllImport("user32.dll", SetLastError = true)] private static extern bool UpdateLayeredWindow(IntPtr hwnd, IntPtr destinationDc, ref POINT destination, ref SIZE size, IntPtr sourceDc, ref POINT source, int colorKey, ref BLENDFUNCTION blend, uint flags);
    [DllImport("user32.dll")] private static extern IntPtr DefWindowProc(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] private static extern void PostQuitMessage(int exitCode);
    [DllImport("user32.dll")] private static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] private static extern IntPtr GetDC(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern int ReleaseDC(IntPtr hwnd, IntPtr dc);
    [DllImport("gdi32.dll")] private static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")] private static extern bool DeleteDC(IntPtr dc);
    [DllImport("gdi32.dll", SetLastError = true)] private static extern IntPtr CreateDIBSection(IntPtr dc, ref BITMAPINFO info, uint usage, out IntPtr bits, IntPtr section, uint offset);
    [DllImport("gdi32.dll")] private static extern IntPtr SelectObject(IntPtr dc, IntPtr objectHandle);
    [DllImport("gdi32.dll")] private static extern bool DeleteObject(IntPtr objectHandle);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] private static extern IntPtr GetModuleHandle(string? moduleName);
}
