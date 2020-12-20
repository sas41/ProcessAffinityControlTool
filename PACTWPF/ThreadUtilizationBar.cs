using System;
using System.Collections.Generic;
using System.Text;
using System.Diagnostics;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace PACTWPF
{
    class ThreadUtilizationBar : ProgressBar
    {
        private int AssociatedThreadNumber { get; set; }
        private PerformanceCounter BoundCounter { get; set; }

        private static TimeSpan duration = TimeSpan.FromMilliseconds(1000);
        public ThreadUtilizationBar(int threadNumber) : base()
        {
            Orientation = Orientation.Vertical;
            AssociatedThreadNumber = threadNumber;
            this.Name = $"Status_ProgressBar_CPU_{AssociatedThreadNumber}";
            this.ToolTip = $"Thread: {AssociatedThreadNumber}";
            BoundCounter = new PerformanceCounter("Processor", "% Processor Time", $"{AssociatedThreadNumber}");
        }

        public void SetPercent(double percentage)
        {
            DoubleAnimation animation = new DoubleAnimation(percentage, duration);
            this.BeginAnimation(ProgressBar.ValueProperty, animation);
        }

        public void UpdateUtilization()
        {
            this.SetPercent(BoundCounter.NextValue());
        }
    }
}
