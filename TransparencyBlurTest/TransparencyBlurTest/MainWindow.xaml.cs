using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.Graphics.Canvas.Effects;
using System;
using System.Numerics;
using Windows.UI;
using WinRT.Interop;
using System.Runtime.InteropServices;

namespace TransparencyBlurTest;

public sealed partial class MainWindow : Window
{
    private Compositor _compositor;
    private SpriteVisual _backgroundVisual;
    private CompositionEffectBrush _effectBrush;

    // DWM constants
    private const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
    private const int DWMSBT_NONE = 0;

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);

    public MainWindow()
    {
        this.InitializeComponent();

        // 1. Setup Composition
        _compositor = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(RootGrid).Compositor;

        // 2. Create the Background Effect (Just Blur for now)
        CreateBackgroundEffect();

        // 3. Create the SpriteVisual for the background
        _backgroundVisual = _compositor.CreateSpriteVisual();
        _backgroundVisual.Brush = _effectBrush;

        // 4. Set as child visual (this puts it BEHIND the XAML content)
        Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.SetElementChildVisual(RootGrid, _backgroundVisual);

        // 5. Handle resizing
        RootGrid.SizeChanged += (s, e) =>
        {
            _backgroundVisual.Size = new Vector2((float)RootGrid.ActualWidth, (float)RootGrid.ActualHeight);
        };

        // Initial size
        _backgroundVisual.Size = new Vector2((float)RootGrid.ActualWidth, (float)RootGrid.ActualHeight);

        // 6. Make the window background transparent at the system level
        MakeWindowTransparent();
    }

    private void CreateBackgroundEffect()
    {
        // Backdrop brush samples the content behind the window
        var backdropBrush = _compositor.CreateBackdropBrush();

        // Gaussian Blur Effect
        var blurEffect = new GaussianBlurEffect
        {
            BlurAmount = 15.0f,
            Source = new CompositionEffectSourceParameter("backdrop")
        };

        var effectFactory = _compositor.CreateEffectFactory(blurEffect);
        _effectBrush = effectFactory.CreateBrush();

        // Link the backdrop brush to the "backdrop" parameter
        _effectBrush.SetSourceParameter("backdrop", backdropBrush);
    }

    private void MakeWindowTransparent()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);

        // Disable system backdrop
        int backdropType = DWMSBT_NONE;
        DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, ref backdropType, sizeof(int));
    }
}
