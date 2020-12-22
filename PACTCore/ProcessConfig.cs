using Newtonsoft.Json;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;

namespace PACTCore
{

    public class ProcessConfig
    {
        [JsonProperty]
        public ProcessPriorityClass Priority { get; set; }

        [JsonProperty]
        public long AffinityMask { get; set; }


        // Affinity mask is calculated using this set of numbers.
        [JsonProperty]
        public List<int> CoreList { get; private set; }



        public ProcessConfig(List<int> coreNumbers = null)
        {
            Priority = ProcessPriorityClass.Normal;

            if (coreNumbers == null)
            {
                CoreList = new List<int>();
            }
            else
            {

                CoreList = coreNumbers;
                ReCalculateMask();
            }
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
