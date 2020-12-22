using System;
using System.Windows.Threading;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using System.Diagnostics;
using PACTCore;
using Newtonsoft.Json;

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        static MainWindow()
        {
            PACTInstance.ToggleProcessOverwatch();
        }



        private List<ThreadUtilizationBar> ThreadBars { get; set; }
        private PerformanceCounter TotalCPUUsage;
        private static DispatcherTimer PerformanceStatisticsUpdateTimer;



        public MainWindow()
        {
            ThreadBars = new List<ThreadUtilizationBar>();
            TotalCPUUsage = new PerformanceCounter("Processor", "% Processor Time", "_Total");
            InitializeComponent();
            InitializePerformanceStatisticsUpdateTimer();
        }



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
                    if (assigned > threadCount)
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

        public void InitializePerformanceStatisticsUpdateTimer()
        {
            PerformanceStatisticsUpdateTimer = new System.Windows.Threading.DispatcherTimer(DispatcherPriority.Render);
            PerformanceStatisticsUpdateTimer.Interval = new TimeSpan(0, 0, 0, 0, 1000);
            PerformanceStatisticsUpdateTimer.Tick += new EventHandler(UpdatePerformanceStatistics);
            PerformanceStatisticsUpdateTimer.Start();
        }

        private void UpdatePerformanceStatistics(Object source, EventArgs e)
        {
            // Update CPU Utilization Values
            foreach (var bar in ThreadBars)
            {
                bar.UpdateUtilization();
            }

            Total_CPU_Usage_Label.Content = $"{TotalCPUUsage.NextValue().ToString("0")}%";


            // It's 8 AM, haven't slept yet.
            // Todo: Find a better way to calculate these.
            var allRunningProcesses = PACTInstance.GetRunningProcesses().Select(x => x.ToLower()).ToList();

            var highPriorityProcesses = allRunningProcesses.Intersect(PACTInstance.GetHighPriorityProcesses().Select(x => x.ToLower())).Count();
            var exceptionPriorityProcesses = allRunningProcesses.Intersect(PACTInstance.GetCustomProcesses().Select(x => x.ToLower())).Count();
            var inaccessibleProcesses = PACTInstance.GetProtectedProcesses().Count();

            Total_Process_Count_Label.Content = allRunningProcesses.Count;
            Active_High_Priority_Process_Count_Label.Content = highPriorityProcesses;
            Active_Exceptions_Count_Label.Content = exceptionPriorityProcesses;
            Inaccessible_Process_Count_Label.Content = inaccessibleProcesses;
        }

        ////////////////////////////////////////////////////////////////////////////

        private void ListBox_HighPriority_Initialized(object sender, EventArgs e)
        {
            ListBox_HighPriority.Items.Clear();
            foreach (var hpp in PACTInstance.GetHighPriorityProcesses())
            {
                ListBox_HighPriority.Items.Add(hpp);
            }
        }

        private void ListBox_HighPriority_SelectionChanged(object sender, EventArgs e)
        {
            Button_HighPriority_Remove.IsEnabled = true;
            Button_HighPriority_MoveToException.IsEnabled = true;
        }

        private void Button_HighPriority_Configure_Click(object sender, RoutedEventArgs e)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow();
            window.TargetProcessOrGroup = "[High Priority Processes]";
            ProcessConfig conf;
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                PACTInstance.UpdateHighPriorityProcessConfig(conf);
            }
        }

        private void Button_HighPriority_MoveToException_Click(object sender, RoutedEventArgs e)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow();
            window.TargetProcessOrGroup = ListBox_HighPriority.SelectedItem.ToString();
            ProcessConfig conf;
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                PACTInstance.AddToCustomPriority(window.TargetProcessOrGroup, conf);
            }
            TriggerListUpdate();
        }

        private void Button_HighPriority_AddManual_Click(object sender, RoutedEventArgs e)
        {
            ProcessNameEntryWindow window = new ProcessNameEntryWindow();
            if (window.ShowDialog() == true)
            {
                PACTInstance.AddToHighPriority(window.ProcessName);
            }
            TriggerListUpdate();
        }

        private void Button_HighPriority_Remove_Click(object sender, RoutedEventArgs e)
        {
            PACTInstance.Clear(ListBox_HighPriority.SelectedItem.ToString());
            TriggerListUpdate();
        }

        private void ListBox_HighPriority_Validate()
        {
            if (ListBox_HighPriority.SelectedItem == null)
            {
                Button_HighPriority_MoveToException.IsEnabled = false;
                Button_HighPriority_Remove.IsEnabled = false;
            }
        }

        ////////////////////////////////////////////////////////////////////////////

        private void ListBox_Normal_Initialized(object sender, EventArgs e)
        {
            ListBox_Normal.Items.Clear();
            foreach (var hpp in PACTInstance.GetRunningProcesses().Distinct())
            {
                ListBox_Normal.Items.Add(hpp);
            }
        }

        private void ListBox_Normal_SelectionChanged(object sender, EventArgs e)
        {
            Button_Normal_MoveToHighPriority.IsEnabled = true;
            Button_Normal_MoveToException.IsEnabled = true;
        }

        private void Button_Normal_Refresh_Click(object sender, RoutedEventArgs e)
        {
            TriggerListUpdate();
        }

        private void Button_Normal_Configure_Click(object sender, RoutedEventArgs e)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow();
            window.TargetProcessOrGroup = "[Normal Priority Processes]";
            ProcessConfig conf;
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                PACTInstance.UpdateDefaultPriorityProcessConfig(conf);
            }

            TriggerListUpdate();
        }

        private void Button_Normal_MoveToException_Click(object sender, RoutedEventArgs e)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow();
            window.TargetProcessOrGroup = ListBox_Normal.SelectedItem.ToString();
            ProcessConfig conf;
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                PACTInstance.AddToCustomPriority(window.TargetProcessOrGroup, conf);
            }

            TriggerListUpdate();
        }

        private void Button_Normal_MoveToHighPriority_Click(object sender, RoutedEventArgs e)
        {
            PACTInstance.AddToHighPriority(ListBox_Normal.SelectedItem.ToString());
            TriggerListUpdate();
        }

        private void ListBox_Normal_Validate()
        {
            if (ListBox_Normal.SelectedItem == null)
            {
                Button_Normal_MoveToException.IsEnabled = false;
                Button_Normal_MoveToHighPriority.IsEnabled = false;
            }
        }

        ////////////////////////////////////////////////////////////////////////////

        private void ListBox_Exceptions_Initialized(object sender, EventArgs e)
        {
            ListBox_Exceptions.Items.Clear();
            foreach (var hpp in PACTInstance.GetCustomProcesses())
            {
                ListBox_Exceptions.Items.Add(hpp);
            }
        }

        private void ListBox_Exceptions_SelectionChanged(object sender, EventArgs e)
        {
            Button_Exceptions_Configure.IsEnabled = true;
            Button_Exceptions_MoveToHighPriority.IsEnabled = true;
            Button_Exceptions_Remove.IsEnabled = true;
        }

        private void Button_Exceptions_Configure_Click(object sender, RoutedEventArgs e)
        {
            ProcessConfigEditWindow window = new ProcessConfigEditWindow();
            window.TargetProcessOrGroup = ListBox_Exceptions.SelectedItem.ToString();
            ProcessConfig conf;
            if (window.ShowDialog() == true)
            {
                conf = window.GenerateConfig();
                PACTInstance.UpdateCustomPriorityProcessConfig(ListBox_Exceptions.SelectedItem.ToString(), conf);
            }
            TriggerListUpdate();
        }

        private void Button_Exceptions_MoveToHighPriority_Click(object sender, RoutedEventArgs e)
        {
            PACTInstance.AddToHighPriority(ListBox_Exceptions.SelectedItem.ToString());
            TriggerListUpdate();
        }

        private void Button_Exceptions_Remove_Click(object sender, RoutedEventArgs e)
        {
            PACTInstance.Clear(ListBox_Exceptions.SelectedItem.ToString());
            TriggerListUpdate();
        }

        private void ListBox_Exceptions_Validate()
        {
            if (ListBox_Exceptions.SelectedItem == null)
            {
                Button_Exceptions_Configure.IsEnabled = false;
                Button_Exceptions_MoveToHighPriority.IsEnabled = false;
                Button_Exceptions_Remove.IsEnabled = false;
            }
        }

        ////////////////////////////////////////////////////////////////////////////

        private void TriggerListUpdate()
        {
            ListBox_HighPriority_Initialized(this, null);
            ListBox_Normal_Initialized(this, null);
            ListBox_Exceptions_Initialized(this, null);

            ListBox_HighPriority_Validate();
            ListBox_Normal_Validate();
            ListBox_Exceptions_Validate();
        }

    }
}
