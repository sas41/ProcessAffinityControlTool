using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Threading;
using PACTCore;

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>

    public partial class MainWindow : Window
    {
        private static PACTInstance pact;
        private static DispatcherTimer UIUpdateTimer;

        private System.Windows.Forms.NotifyIcon TrayIcon;
        private List<ThreadUtilizationBar> ThreadBars { get; set; }

        private PerformanceCounter TotalCPUUsage;

        public MainWindow()
        {
            pact = new PACTInstance();
            //pact.ToggleProcessOverwatch();

            ThreadBars = new List<ThreadUtilizationBar>();
            TotalCPUUsage = new PerformanceCounter("Processor", "% Processor Time", "_Total");

            InitializeComponent();
            InitializeUIUpdateTimer();
            InitTrayIcon();

            pact.ConfigUpdated += UpdatePerformanceBarColors;
            UpdatePerformanceBarColors(this, EventArgs.Empty);

            var startMinimized = (Application.Current as App).StartMinimized;
            if (startMinimized)
            {
                Button_MinimizeToTray_Click(this, new RoutedEventArgs());
            }
        }

        private void Label_Title_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ChangedButton == MouseButton.Left)
                if (e.ClickCount == 2)
                {
                    AdjustWindowSize();
                }
                else
                {
                    Application.Current.MainWindow.DragMove();
                }
        }

        private void Button_MinimizeToTray_Click(object sender, RoutedEventArgs e)
        {
            this.WindowState = WindowState.Minimized;
            this.Hide();
            TrayIcon.ShowBalloonTip(5000);
        }

        private void Button_Minimize_Click(object sender, RoutedEventArgs e)
        {
            this.WindowState = WindowState.Minimized;
        }

        private void Button_Maximize_Click(object sender, RoutedEventArgs e)
        {
            AdjustWindowSize();
        }

        private void Button_Close_Click(object sender, RoutedEventArgs e)
        {
            pact.SaveConfig();
            Application.Current.Shutdown();
        }

        private void AdjustWindowSize()
        {
            if (this.WindowState == WindowState.Maximized)
            {
                this.WindowState = WindowState.Normal;
            }
            else
            {
                this.WindowState = WindowState.Maximized;
            }
        }

        public void InitTrayIcon()
        {
            TrayIcon = new System.Windows.Forms.NotifyIcon();
            TrayIcon.Icon = System.Drawing.Icon.ExtractAssociatedIcon($"{AppDomain.CurrentDomain.BaseDirectory}/{Process.GetCurrentProcess().ProcessName}.exe");
            TrayIcon.Text = "Click to bring PACT back.";
            TrayIcon.Visible = true;

            TrayIcon.BalloonTipIcon = new System.Windows.Forms.ToolTipIcon();
            TrayIcon.BalloonTipTitle = "PACT for Windows";
            TrayIcon.BalloonTipText = "PACT is minimized to tray.";

            TrayIcon.Click += TrayIcon_Clicked;
        }

        public void TrayIcon_Clicked(object sender, EventArgs args)
        {
            this.Show();
            this.WindowState = WindowState.Normal;
        }

        private void Window_StateChanged(object sender, EventArgs e)
        {
            if (this.WindowState == WindowState.Minimized)
            {
                UIUpdateTimer.Stop();
            }
            else
            {
                UIUpdateTimer.Start();
            }
        }

        private void Window_Closed(object sender, EventArgs e)
        {
            TrayIcon.Dispose();
        }

        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////
        ////                            Status Tab                              ////
        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////

        private void Grid_Status_CPU_Initialized(object sender, EventArgs e)
        {
            int threadCount = Environment.ProcessorCount;

            int columns = 2;
            int rows = 1;
            int gridSize = columns * rows;

            while (gridSize < threadCount)
            {
                if (rows <= columns)
                {
                    rows++;
                }
                else
                {
                    columns += 2;
                }

                gridSize = columns * rows;
            }

            for (int i = 0; i < columns; i++)
            {
                Grid_Status_CPU.ColumnDefinitions.Add(new ColumnDefinition());
            }

            for (int i = 0; i < rows; i++)
            {
                Grid_Status_CPU.RowDefinitions.Add(new RowDefinition());
            }

            int assigned = 0;
            for (int i = 0; i < rows; i++)
            {
                for (int j = 0; j < columns; j++)
                {
                    if (assigned >= threadCount)
                    {
                        break;
                    }

                    ThreadUtilizationBar tub = new ThreadUtilizationBar(assigned);
                    Grid.SetRow(tub, i);
                    Grid.SetColumn(tub, j);
                    ThreadBars.Add(tub);

                    Grid.SetRow(tub.CustomLabel, i);
                    Grid.SetColumn(tub.CustomLabel, j);

                    assigned++;

                    Grid_Status_CPU.Children.Add(tub);
                    Grid_Status_CPU.Children.Add(tub.CustomLabel);
                }
            }
        }

        public void InitializeUIUpdateTimer()
        {
            UIUpdateTimer = new System.Windows.Threading.DispatcherTimer(DispatcherPriority.Render);
            UIUpdateTimer.Interval = new TimeSpan(0, 0, 0, 0, 2000);
            UIUpdateTimer.Tick += new EventHandler(UpdatePerformanceStatistics);
            UIUpdateTimer.Tick += new EventHandler(TriggerSettingsTabListUpdate);
            UIUpdateTimer.Tick += new EventHandler(TriggerAutoModeTabListUpdate);

            UIUpdateTimer.Start();
        }

        private void UpdatePerformanceBarColors(object source, EventArgs e)
        {
            var highs = pact.PACTProcessOverwatch.ActiveConfig.HighPerformanceProcessConfig.CoreList;
            var normals = pact.PACTProcessOverwatch.ActiveConfig.DefaultPerformanceProcessConfig.CoreList;
            for (int i = 0; i < ThreadBars.Count; i++)
            {
                var bar = ThreadBars[i];
                bar.AutoSetColor(normals.Contains(i), highs.Contains(i));
            }
        }

        private void UpdatePerformanceStatistics(Object source, EventArgs e)
        {
            // Update CPU Utilization Values
            foreach (var bar in ThreadBars)
            {
                bar.UpdateUtilization();
            }

            Total_CPU_Usage_Label.Content = $"{TotalCPUUsage.NextValue().ToString("0")}%";

            var allRunningProcesses = pact.GetAllRunningProcesses().Select(x => x.ToLower()).ToList();

            var HighPerformanceProcesses = allRunningProcesses.Intersect(pact.GetHighPerformanceProcesses().Select(x => x.ToLower())).Count();
            var exceptionPriorityProcesses = allRunningProcesses.Intersect(pact.GetCustomProcesses().Select(x => x.ToLower())).Count();
            var inaccessibleProcesses = pact.GetProtectedProcesses().Count();

            Total_Process_Count_Label.Content = allRunningProcesses.Count();
            Active_High_Performance_Process_Count_Label.Content = HighPerformanceProcesses;
            Active_Custom_Count_Label.Content = exceptionPriorityProcesses;
            Inaccessible_Process_Count_Label.Content = inaccessibleProcesses;
        }

        private void Button_Toggle_Click(object sender, RoutedEventArgs e)
        {
            if (pact.ToggleProcessOverwatch())
            {
                Label_ToggleStatus.Content = "ACTIVE";
                Label_ToggleStatus.Foreground = System.Windows.Media.Brushes.Green;
            }
            else
            {
                Label_ToggleStatus.Content = "PAUSED";
                Label_ToggleStatus.Foreground = System.Windows.Media.Brushes.Red;
            }
        }

        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////
        ////                           Configure Tab                            ////
        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////
        
        private ProcessConfig OpenProcessConfigWindow(string target, ProcessConfig initial)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow(initial);
            window.TargetProcessOrGroup = target;
            ProcessConfig conf = null;

            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
            }

            window.Close();
            return conf;
        }

        private ProcessConfig OpenProcessConfigWindow(out string name)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow(new ProcessConfig());
            window.TargetProcessOrGroup = "";
            ProcessConfig conf = null;

            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
            }

            name = window.TargetProcessOrGroup;
            window.Close();
            return conf;
        }

        private void DragCheck(ListView source, MouseEventArgs mouseEvent)
        {
            System.Windows.Point mousePosition = mouseEvent.GetPosition(null);

            bool movedX = Math.Abs(mousePosition.X) > SystemParameters.MinimumHorizontalDragDistance;
            bool movedY = Math.Abs(mousePosition.Y) > SystemParameters.MinimumVerticalDragDistance;
            bool hasMoved = movedX || movedY;

            bool posX = mouseEvent.GetPosition(source).X < source.ActualWidth - SystemParameters.VerticalScrollBarWidth;
            bool posY = mouseEvent.GetPosition(source).Y < source.ActualHeight - SystemParameters.HorizontalScrollBarHeight;
            bool isOnTarget = posX && posY;

            bool lmbIsDown = mouseEvent.LeftButton == MouseButtonState.Pressed;

            if (lmbIsDown && hasMoved && isOnTarget)
            {
                List<string> items = new List<string>();
                foreach (var item in source.SelectedItems)
                {
                    items.Add(item.ToString());
                }
                // God what a pain in the ass Drag and Drop is.
                // This hack here was the simplest solution.
                // If anyone has enough experience with WPF,
                // I am open to suggestions.
                items.Add(source.Name);

                if (items.Count > 0)
                {
                    DragDrop.DoDragDrop(source, string.Join(Environment.NewLine, items), DragDropEffects.Copy | DragDropEffects.Move);
                }
            }
        }

        private void DropTrigger(ListView destination, DragEventArgs dragEvent, Action<List<string>> action)
        {
            if (dragEvent.Data.GetDataPresent(DataFormats.StringFormat))
            {
                string dataString = (string)dragEvent.Data.GetData(DataFormats.StringFormat);
                var itemList = dataString.Split(Environment.NewLine, StringSplitOptions.RemoveEmptyEntries).ToList();
                var sourceName = itemList.Last();
                itemList.RemoveAt(itemList.Count - 1);
                if (sourceName != destination.Name && itemList.Count > 0)
                {
                    action(itemList);
                }
            }

            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        bool ProcessSearchFilter(object obj)
        {
            if (Configure_Search.Text != null && Configure_Search.Text != "")
            {
                return (obj as string).ToLower().StartsWith(Configure_Search.Text.ToLower());
            }
            else
            {
                return true;
            }
        }

        private void Configure_Search_TextChanged(object sender, TextChangedEventArgs e)
        {
            // Setting the filter triggers also applies it immediately.
            ListView_Normal.Items.Filter = ProcessSearchFilter;
            ListView_HighPerformance.Items.Filter = ProcessSearchFilter;
            ListView_Custom.Items.Filter = ProcessSearchFilter;
            ListView_Blacklist.Items.Filter = ProcessSearchFilter;
        }

        private void Button_Refresh_Click(object sender, RoutedEventArgs e)
        {
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void TriggerSettingsTabListUpdate(Object source, EventArgs e)
        {
            ListView_Normal_Initialized(this, null);
            ListView_HighPerformance_Initialized(this, null);
            ListView_Custom_Initialized(this, null);
            ListView_Blacklist_Initialized(this, null);

            if (ListView_Custom.SelectedItem == null)
            {
                Button_Custom_Configure.IsEnabled = false;
            }
        }

        ////////////////////////////////////////////////////////////////////////////
        //                     Normal Performance Column                          //
        ////////////////////////////////////////////////////////////////////////////

        private void ListView_Normal_Initialized(object sender, EventArgs e)
        {
            ListView_Normal.ItemsSource = pact.GetNormalPerformanceProcesses().ToList();
            ListView_Normal.Items.Filter = ProcessSearchFilter;
        }

        private void ListView_Normal_MouseMove(object sender, MouseEventArgs e)
        {
            DragCheck(ListView_Normal, e);
        }

        private void ListView_Normal_Drop(object sender, DragEventArgs e)
        {
            DropTrigger(ListView_Normal, e, pact.ClearProcesses);
        }

        private void Button_Normal_Configure_Click(object sender, RoutedEventArgs e)
        {
            var conf = OpenProcessConfigWindow("[Normal Performance Processes]", pact.GetDefaultPerformanceConfig());
            if (conf != null)
            {
                pact.UpdateDefaultPerformanceProcessConfig(conf);
            }
        }

        ////////////////////////////////////////////////////////////////////////////
        //                       High Performance Column                          //
        ////////////////////////////////////////////////////////////////////////////
        
        private void ListView_HighPerformance_Initialized(object sender, EventArgs e)
        {
            ListView_HighPerformance.ItemsSource = pact.GetHighPerformanceProcesses().ToList();
            ListView_HighPerformance.Items.Filter = ProcessSearchFilter;
        }

        private void ListView_HighPerformance_MouseMove(object sender, MouseEventArgs e)
        {
            DragCheck(ListView_HighPerformance, e);
        }

        private void ListView_HighPerformance_Drop(object sender, DragEventArgs e)
        {
            DropTrigger(ListView_HighPerformance, e, pact.AddToHighPerformance);
        }

        private void Button_HighPerformance_Add_Click(object sender, RoutedEventArgs e)
        {
            ProcessNameEntryWindow window = new ProcessNameEntryWindow();
            if (window.ShowDialog() == true)
            {
                pact.AddToHighPerformance(window.ProcessName);
            }
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_HighPerformance_Configure_Click(object sender, RoutedEventArgs e)
        {
            var conf = OpenProcessConfigWindow("[High Performance Processes]", pact.GetHighPerformanceConfig());
            if (conf != null)
            {
                pact.UpdateHighPerformanceProcessConfig(conf);
            }
        }

        ////////////////////////////////////////////////////////////////////////////
        //                       Custom Performance Column                        //
        ////////////////////////////////////////////////////////////////////////////

        private void ListView_Custom_Initialized(object sender, EventArgs e)
        {
            ListView_Custom.ItemsSource = pact.GetCustomProcesses().ToList();
            ListView_Custom.Items.Filter = ProcessSearchFilter;
        }

        private void ListView_Custom_SelectionChanged(object sender, EventArgs e)
        {
            Button_Custom_Configure.IsEnabled = true;
        }

        private void ListView_Custom_MouseMove(object sender, MouseEventArgs e)
        {
            DragCheck(ListView_Custom, e);
        }

        private void ListView_Custom_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.StringFormat))
            {
                string dataString = (string)e.Data.GetData(DataFormats.StringFormat);
                var itemList = dataString.Split(Environment.NewLine, StringSplitOptions.RemoveEmptyEntries).ToList();
                var sourceName = itemList.Last();
                itemList.RemoveAt(itemList.Count - 1);
                if (sourceName != ListView_Custom.Name && itemList.Count > 0)
                {
                    var conf = OpenProcessConfigWindow(string.Join(", ", itemList), new ProcessConfig());

                    if (conf != null)
                    {
                        pact.AddToCustomPriority(itemList, conf);
                    }
                    TriggerSettingsTabListUpdate(null, EventArgs.Empty);
                }
            }
        }

        private void Button_Custom_Add_Click(object sender, RoutedEventArgs e)
        {
            string name;
            var conf = OpenProcessConfigWindow(out name);
            if (conf != null)
            {
                pact.AddToCustomPriority(name, conf);
            }

            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Custom_Configure_Click(object sender, RoutedEventArgs e)
        {
            string target = ListView_Custom.SelectedItem.ToString();
            ProcessConfig initial = new ProcessConfig();
            if (pact.PACTProcessOverwatch.UserConfig.CustomPerformanceProcesses.ContainsKey(target))
            {
                initial = pact.PACTProcessOverwatch.UserConfig.CustomPerformanceProcesses[target];
            }

            ProcessConfigEditWindow window = new ProcessConfigEditWindow(initial);
            window.TargetProcessOrGroup = ListView_Custom.SelectedItem.ToString();
            ProcessConfig conf = new ProcessConfig();
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                pact.AddToCustomPriority(ListView_Custom.SelectedItem.ToString(), conf);
            }
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        ////////////////////////////////////////////////////////////////////////////
        //                           Blacklist Column                             //
        ////////////////////////////////////////////////////////////////////////////

        private void ListView_Blacklist_Initialized(object sender, EventArgs e)
        {
            ListView_Blacklist.ItemsSource = pact.GetBlacklistedProcesses().ToList();
            ListView_Blacklist.Items.Filter = ProcessSearchFilter;
        }

        private void ListView_Blacklist_MouseMove(object sender, MouseEventArgs e)
        {
            DragCheck(ListView_Blacklist, e);
        }

        private void ListView_Blacklist_Drop(object sender, DragEventArgs e)
        {
            DropTrigger(ListView_Blacklist, e, pact.AddToBlacklist);
        }

        private void Button_Blacklist_Add_Click(object sender, RoutedEventArgs e)
        {
            ProcessNameEntryWindow window = new ProcessNameEntryWindow();
            if (window.ShowDialog() == true)
            {
                pact.AddToBlacklist(window.ProcessName);
            }
            window.Close();
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////
        ////                            AutoMode Tab                            ////
        ////////////////////////////////////////////////////////////////////////////
        ////////////////////////////////////////////////////////////////////////////

        private void TriggerAutoModeTabListUpdate(Object source, EventArgs e)
        {
            ListView_AutoModeDetections_Initialized(this, null);

            if (ListView_AutoModeLaunchers.SelectedItem == null)
            {
                Button_AutoMode_Remove.IsEnabled = false;
            }
        }

        private void Button_AutoMode_Toggle_Click(object sender, RoutedEventArgs e)
        {
            if (pact.ToggleAutoMode())
            {
                Label_AutoMode.Content = "AUTO MODE ON";
                Label_AutoMode.Foreground = System.Windows.Media.Brushes.Green;
            }
            else
            {
                Label_AutoMode.Content = "AUTO MODE OFF";
                Label_AutoMode.Foreground = System.Windows.Media.Brushes.Red;
            }
        }

        private void Button_AutoMode_Add_Click(object sender, RoutedEventArgs e)
        {
            ProcessNameEntryWindow window = new ProcessNameEntryWindow();
            if (window.ShowDialog() == true)
            {
                pact.AddToAutoModeLaunchers(window.ProcessName);
            }

            ListView_AutoModeLaunchers_Initialized(this, null);
        }

        private void Button_AutoMode_Remove_Click(object sender, RoutedEventArgs e)
        {
            if (ListView_AutoModeLaunchers.SelectedItem != null)
            {
                pact.RemoveFromAutoModeLaunchers(ListView_AutoModeLaunchers.SelectedItem.ToString());
            }

            ListView_AutoModeLaunchers_Initialized(this, null);
        }

        private void ListView_AutoModeLaunchers_SelectionChanged(object sender, EventArgs e)
        {
            Button_AutoMode_Remove.IsEnabled = true;
        }

        private void ListView_AutoModeLaunchers_Initialized(object sender, EventArgs e)
        {
            ListView_AutoModeLaunchers.ItemsSource = pact.GetAutoModeLaunchers().ToList();
        }

        private void ListView_AutoModeDetections_Initialized(object sender, EventArgs e)
        {
            ListView_AutoModeDetections.ItemsSource = pact.GetAutoModeDetections().ToList();
        }

        ////////////////////////////////////////////////////////////////////////////
        //                          Settings/Options Tab                          //
        ////////////////////////////////////////////////////////////////////////////

        private string OpenFile(string defaultExt, string filter)
        {
            System.Windows.Forms.OpenFileDialog dialog = new System.Windows.Forms.OpenFileDialog();

            dialog.DefaultExt = defaultExt;
            dialog.Filter = filter;
            dialog.RestoreDirectory = true;

            if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK)
            {
                return dialog.FileName;
            }

            return "";
        }

        private string SaveFile(string defaultExt, string filter)
        {
            System.Windows.Forms.SaveFileDialog dialog = new System.Windows.Forms.SaveFileDialog();

            dialog.DefaultExt = defaultExt;
            dialog.Filter = filter;
            dialog.RestoreDirectory = true;

            if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK)
            {
                return dialog.FileName;
            }

            return "";
        }

        private void Button_Options_HighPriority_Import_Click(object sender, RoutedEventArgs e)
        {
            string path = OpenFile(".txt", "Text Files (*.txt)|*.txt");
            if (path != "")
            {
                pact.ImportHighPerformance(path);
            }
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Options_HighPriority_Export_Click(object sender, RoutedEventArgs e)
        {
            string path = SaveFile(".txt", "Text Files (*.txt)|*.txt");
            if (path != "")
            {
                pact.ExportHighPerformance(path);
            }
        }

        private void Button_Options_HighPriority_Clear_Click(object sender, RoutedEventArgs e)
        {
            pact.ClearHighPerformance();
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Options_Blacklist_Import_Click(object sender, RoutedEventArgs e)
        {
            string path = OpenFile(".txt", "Text Files (*.txt)|*.txt");
            if (path != "")
            {
                pact.ImportBlacklist(path);
            }
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Options_Blacklist_Export_Click(object sender, RoutedEventArgs e)
        {
            string path = SaveFile(".txt", "Text Files (*.txt)|*.txt");
            if (path != "")
            {
                pact.ExportBlacklist(path);
            }
        }

        private void Button_Options_Blacklist_Clear_Click(object sender, RoutedEventArgs e)
        {
            pact.ClearBlackList();
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Options_Config_Import_Click(object sender, RoutedEventArgs e)
        {
            string path = OpenFile(".json", "JSON Files (*.json)|*.json");
            if (path != "")
            {
                pact.ImportConfig(path);
            }
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
            // I don't understand why (yet), but for some reason
            // the performance bars do not update if Reset
            // or Import config buttons are pressed.
            // This harmless hack ensures they update properly. 
            pact.ToggleProcessOverwatch();
            pact.ToggleProcessOverwatch();
        }

        private void Button_Options_Config_Export_Click(object sender, RoutedEventArgs e)
        {
            string path = SaveFile(".json", "JSON Files (*.json)|*.json");
            if (path != "")
            {
                pact.ExportConfig(path);
            }
        }

        private void Button_Options_Custom_Clear_Click(object sender, RoutedEventArgs e)
        {
            pact.ClearCustoms();
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
        }

        private void Button_Options_About_Click(object sender, RoutedEventArgs e)
        {
            OpenURL("https://github.com/sas41/ProcessAffinityControlTool#readme");
        }

        private void Button_Options_ResetConfig_Click(object sender, RoutedEventArgs e)
        {
            pact.ResetConfig();
            TriggerSettingsTabListUpdate(null, EventArgs.Empty);
            // I don't understand why (yet), but for some reason
            // the performance bars do not update if Reset
            // or Import config buttons are pressed.
            // This harmless hack ensures they update properly. 
            pact.ToggleProcessOverwatch();
            pact.ToggleProcessOverwatch();
        }

        private void OpenURL(string url)
        {
            try
            {
                Process.Start(url);
            }
            catch
            {
                // hack because of this: https://github.com/dotnet/corefx/issues/10361
                if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
                {
                    url = url.Replace("&", "^&");
                    Process.Start(new ProcessStartInfo("cmd", $"/c start {url}") { CreateNoWindow = true });
                }
                else if (RuntimeInformation.IsOSPlatform(OSPlatform.Linux))
                {
                    Process.Start("xdg-open", url);
                }
                else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
                {
                    Process.Start("open", url);
                }
                else
                {
                    throw;
                }
            }
        }

    }
}