using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;

namespace ProcessAffinityControlTool
{
    class ProcessConfig
    {
        public ProcessPriorityClass Priority { get; set; }
        public long AffinityMask { get; set; }

        public List<int> CoreList { get; private set; }
        public int PriorityNumber { get; private set; }

        public ProcessConfig()
        {
            // Empty Constructor for JSON Deserialization, do not use!
            CoreList = new List<int>();
        }

        public ProcessConfig(List<int> coreNumbers, int priority)
        {
            CoreList = coreNumbers;
            PriorityNumber = priority;

            AffinityMask = CalculateMask(coreNumbers);
            Priority = CalculatePriority(priority);
        }

        public static long CalculateMask(List<int> coreNumbers)
        {
            long mask = 0;

            long maxCores = Environment.ProcessorCount;
            if (coreNumbers.Any(number => number > maxCores))
            {
                throw new InvalidOperationException($"Invalid Core number. Max number of cores: {maxCores}");
            }

            foreach (var coreNumber in coreNumbers)
            {
                mask = mask | (1 << coreNumber);
            }

            return mask;
        }

        public static ProcessPriorityClass CalculatePriority(int priority)
        {
            switch (priority)
            {
                case 0: { return ProcessPriorityClass.Idle; }
                case 1: { return ProcessPriorityClass.BelowNormal; }
                case 2: { return ProcessPriorityClass.Normal; }
                case 3: { return ProcessPriorityClass.AboveNormal; }
                case 4: { return ProcessPriorityClass.High; }
                case 5: { return ProcessPriorityClass.RealTime; }
                default: { return ProcessPriorityClass.Normal; }
            }
        }

        public override string ToString()
        {
            return $"Priority: {PriorityNumber} ({Priority}), Mask: {AffinityMask}, Cores: {string.Join(", ", CoreList)}";
        }
    }
}
