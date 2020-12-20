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

namespace PACTWPF
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        private List<ThreadUtilizationBar> ThreadBars { get; set; }
        private static DispatcherTimer PerformanceStatisticsUpdateTimer;

        public MainWindow()
        {
            ThreadBars = new List<ThreadUtilizationBar>();
            InitializeComponent();
            InitializePerformanceUpdateTimer();
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
                    assigned++;

                    Grid_Status_CPU.Children.Add(tub);
                }
            }
        }

        public void InitializePerformanceUpdateTimer()
        {
            PerformanceStatisticsUpdateTimer = new System.Windows.Threading.DispatcherTimer(DispatcherPriority.Render);
            PerformanceStatisticsUpdateTimer.Interval = new TimeSpan(0, 0, 0,0,1000);
            PerformanceStatisticsUpdateTimer.Tick += new EventHandler(UpdateCPUUtilizationValues);
            PerformanceStatisticsUpdateTimer.Start();
        }

        private void UpdateCPUUtilizationValues(Object source, EventArgs e)
        {
            foreach (var bar in ThreadBars)
            {
                bar.UpdateUtilization();
            }
        }
    }
}
