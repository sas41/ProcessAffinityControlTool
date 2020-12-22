using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text;
using System.Windows.Data;
using System.ComponentModel;

namespace PACTWPF
{
    class CPUUsageDataProvider : DataSourceProvider
    {
        private List<PerformanceCounter> performanceCounters;
        private List<double> threadBusyTimes;

        public List<double> ThreadBusyTimes
        {
            get
            {
                return threadBusyTimes;
            }
            set
            {
                threadBusyTimes = value;
                base.OnPropertyChanged(new PropertyChangedEventArgs("ThreadBusyTimes"));
            }
        }

        public CPUUsageDataProvider() : base()
        {
            performanceCounters = new List<PerformanceCounter>();
            for (int i = 0; i < Environment.ProcessorCount; i++)
            {
                performanceCounters.Add(new PerformanceCounter("Processor", "% Processor Time", $"{i}"));
            }

            ThreadBusyTimes = new List<double>();
            for (int i = 0; i < Environment.ProcessorCount; i++)
            {
                ThreadBusyTimes.Add(performanceCounters[i].NextValue());
            }
        }

        protected override void BeginQuery()
        {
            for (int i = 0; i < Environment.ProcessorCount; i++)
            {
                ThreadBusyTimes[i] = (performanceCounters[i].NextValue());
            }

            base.OnQueryFinished(ThreadBusyTimes);
        }

    }
}
