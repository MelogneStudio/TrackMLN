using Microsoft.UI.Xaml;

namespace TransparencyBlurTest;

public partial class App : Application
{
    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        m_window = new MainWindow();
        m_window.Activate();
    }

    private Window m_window;
}
