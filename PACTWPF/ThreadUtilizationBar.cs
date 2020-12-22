﻿using System;
using System.Collections.Generic;
using System.Text;
using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace PACTWPF
{
    class ThreadUtilizationBar : ProgressBar
    {
        private int AssociatedThreadNumber { get; set; }
        private PerformanceCounter BoundCounter { get; set; }

        public Label CustomLabel { get; set; }

        private static TimeSpan duration = TimeSpan.FromMilliseconds(1000);
        public ThreadUtilizationBar(int threadNumber) : base()
        {
            Orientation = Orientation.Vertical;
            AssociatedThreadNumber = threadNumber;
            this.Name = $"Status_ProgressBar_CPU_{AssociatedThreadNumber}";
            this.ToolTip = $"Thread: {AssociatedThreadNumber}";
            BoundCounter = new PerformanceCounter("Processor", "% Processor Time", $"{AssociatedThreadNumber}");

            CustomLabel = new Label();
            CustomLabel.HorizontalAlignment = HorizontalAlignment.Center;
            CustomLabel.VerticalAlignment = VerticalAlignment.Center;
            CustomLabel.HorizontalContentAlignment = HorizontalAlignment.Center;
            CustomLabel.VerticalContentAlignment = VerticalAlignment.Center;
            CustomLabel.FontSize = CustomLabel.FontSize * 2;
        }

        public void UpdateUtilization()
        {
            double percentage = BoundCounter.NextValue();
            DoubleAnimation animation = new DoubleAnimation(percentage, duration);
            this.BeginAnimation(ProgressBar.ValueProperty, animation);
            CustomLabel.Content = $"Thread {AssociatedThreadNumber}:{Environment.NewLine}{percentage.ToString("0")}%";

        }
    }
}