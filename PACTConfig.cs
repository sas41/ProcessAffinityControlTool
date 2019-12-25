using System;
using System.Text;
using System.Linq;
using System.Collections.Generic;
using System.Diagnostics;

namespace ProcessAffinityControlTool
{
    class PACTConfig
    {
        // Process name -> Affinity Mask
        public Dictionary<string, ProcessConfig> ProcessConfigs { get; set; }

        public ProcessConfig Default { get; set; }
        public int ScanInterval { get; set; }
        public int AggressiveScanInterval { get; set; }
        public bool ForceAggressiveScan { get; set; }

        public PACTConfig()
        {
            ProcessConfigs = new Dictionary<string, ProcessConfig>();
            Default = new ProcessConfig(Enumerable.Range(0, Environment.ProcessorCount).ToList(), 2);
            ScanInterval = 3000;
            AggressiveScanInterval = 3;
            ForceAggressiveScan = false;
        }

    }
}
