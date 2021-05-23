using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PACTCore
{

    public class ProcessConfig
    {
        [JsonInclude]
        public ProcessPriorityClass Priority { get; set; }

        [JsonInclude]
        public long AffinityMask { get; set; }


        [JsonInclude]
        // Affinity mask is calculated using this set of numbers.
        public List<int> CoreList { get; private set; }


        public ProcessConfig()
        {
            CoreList = Enumerable.Range(0, Environment.ProcessorCount).ToList();
            ReCalculateMask();
            Priority = ProcessPriorityClass.Normal;
        }

        public ProcessConfig(List<int> cores, ProcessPriorityClass priority)
        {
            int maxCount = Environment.ProcessorCount;
            if (cores.Any(x => x < 0 || x > maxCount))
            {
                throw new ArgumentOutOfRangeException($"Thread Numbers are between 0 and {maxCount} on this machine!");
            }

            CoreList = cores;
            ReCalculateMask();
            Priority = priority;
        }

        public long ReCalculateMask()
        {
            long mask = 0;

            long maxCores = Environment.ProcessorCount;
            if (CoreList.Any(number => number > maxCores))
            {
                throw new InvalidOperationException($"Invalid Core number. Max number of cores: {maxCores}");
            }
            else if (CoreList.Count == 0)
            {
                CoreList = new List<int>() { 0 };
            }

            foreach (var coreNumber in CoreList)
            {
                mask = mask | (1 << coreNumber);
            }

            AffinityMask = mask;
            return mask;
        }

        public override string ToString()
        {
            return $"Priority: ({Priority.ToString()}), Mask: {AffinityMask}, Cores: {string.Join(", ", CoreList)}";
        }
    }
}
